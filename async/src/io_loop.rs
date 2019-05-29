use amq_protocol::frame::{GenError, Offset, gen_frame, parse_frame};
use log::{error, trace};
use mio::{Evented, Events, Poll, PollOpt, Ready, Registration, SetReadiness, Token};
use mio_extras::timer::{Builder as TimerBuilder, Timer};
use parking_lot::Mutex;

use std::{
  io::{self, Read, Write},
  sync::Arc,
  thread::{Builder as ThreadBuilder, JoinHandle},
  time::Duration,
};

use crate::{
  buffer::Buffer,
  connection::Connection,
  connection_status::ConnectionState,
  error::{Error, ErrorKind},
};

const SOCKET:    Token = Token(0);
const DATA:      Token = Token(1);
const HEARTBEAT: Token = Token(2);
const CONTINUE:  Token = Token(3);

pub struct IoLoop<T> {
  inner: Arc<Mutex<Inner<T>>>,
}

impl<T: Evented + Read + Write + Send + 'static> IoLoop<T> {
  pub fn new(connection: Connection, socket: T) -> Result<Self, Error> {
    Ok(Self {
      inner: Arc::new(Mutex::new(Inner::new(connection, socket)?))
    })
  }

  pub fn run(&self) -> Result<(), Error> {
    let inner  = self.inner.clone();
    let handle = ThreadBuilder::new().name("io_loop".to_owned()).spawn(move || {
      let mut events = Events::with_capacity(1024);
      loop {
        inner.lock().run(&mut events)?;
      }
    }).map_err(ErrorKind::IOError)?;
    self.inner.lock().handle = Some(handle);
    Ok(())
  }
}

#[derive(PartialEq)]
enum Status {
  Initial,
  Setup,
}

struct Inner<T> {
  connection:     Connection,
  socket:         T,
  status:         Status,
  poll:           Poll,
  registration:   Registration,
  set_readiness:  SetReadiness,
  handle:         Option<JoinHandle<Result<(), Error>>>,
  timer:          Option<Timer<()>>,
  capacity:       usize,
  receive_buffer: Buffer,
  send_buffer:    Buffer,
  can_write:      bool,
  can_read:       bool,
  has_data:       bool,
  send_heartbeat: bool,
}

impl<T> Drop for Inner<T> {
  fn drop(&mut self) {
    if let Some(handle) = self.handle.take() {
      handle.join().expect("io loop failed").expect("io loop failed");
    }
  }
}

impl<T: Evented + Read + Write> Inner<T> {
  fn new(connection: Connection, socket: T) -> Result<Self, Error> {
    let capacity = std::cmp::max(8192, connection.configuration.frame_max() as usize);
    let (registration, set_readiness) = Registration::new2();
    let inner = Self {
      connection,
      socket,
      status:         Status::Initial,
      poll:           Poll::new().map_err(ErrorKind::IOError)?,
      registration,
      set_readiness,
      handle:         None,
      timer:          None,
      capacity,
      receive_buffer: Buffer::with_capacity(capacity),
      send_buffer:    Buffer::with_capacity(capacity),
      can_write:      false,
      can_read:       false,
      has_data:       false,
      send_heartbeat: false,
    };
    inner.poll.register(&inner.socket, SOCKET, Ready::readable() | Ready::writable(), PollOpt::edge()).map_err(ErrorKind::IOError)?;
    inner.poll.register(&inner.connection, DATA, Ready::readable(), PollOpt::edge()).map_err(ErrorKind::IOError)?;
    inner.poll.register(&inner.registration, CONTINUE, Ready::readable(), PollOpt::edge()).map_err(ErrorKind::IOError)?;
    Ok(inner)
  }

  fn schedule_heartbeat(&mut self) {
    let heartbeat = self.connection.configuration.heartbeat() as u64;
    if let Some(timer) = self.timer.as_mut() {
      timer.set_timeout(Duration::from_secs(heartbeat / 2), ());
    }
  }

  fn heartbeat(&mut self) -> Result<(), Error> {
    self.connection.send_heartbeat()?;
    self.send_heartbeat = false;
    self.schedule_heartbeat();
    Ok(())
  }

  fn ensure_setup(&mut self) -> Result<(), Error> {
    if self.status != Status::Setup && self.connection.status.connected() {
      let frame_max = self.connection.configuration.frame_max() as usize;
      if frame_max > self.capacity {
        self.capacity = frame_max;
        self.receive_buffer.grow(frame_max);
        self.send_buffer.grow(frame_max);
      }
      if self.connection.configuration.heartbeat() != 0 {
      let timer = TimerBuilder::default().build();
        self.poll.register(&timer, HEARTBEAT, Ready::readable(), PollOpt::edge()).map_err(ErrorKind::IOError)?;
        self.timer = Some(timer);
        self.schedule_heartbeat();
      }
      self.status = Status::Setup;
    }
    Ok(())
  }

  fn wants_to_write(&self) -> bool {
    self.can_write && self.has_data
  }

  fn wants_to_read(&self) -> bool {
    self.can_read
  }

  fn do_run(&mut self) -> Result<(), Error> {
    if self.send_heartbeat {
      self.heartbeat()?;
    }
    if self.wants_to_write() && !self.connection.status.blocked() {
      if let Err(e) = self.write_to_stream() {
        match e.kind() {
          ErrorKind::NoNewMessage => self.has_data = false,
          ErrorKind::IOError(e) if e.kind() == io::ErrorKind::WouldBlock => self.can_write = false,
          ErrorKind::SendBufferTooSmall => { /* We already shifted the buffer and we already limit the size of the frames, so retry */ },
          _ => {
            error!("error writing: {:?}", e);
            self.connection.set_error()?;
            return Err(e);
          }
        }
      }
    }
    if self.wants_to_read() {
      if let Err(e) = self.read_from_stream() {
        match e.kind() {
          ErrorKind::IOError(e) if e.kind() == io::ErrorKind::WouldBlock => self.can_read = false,
          _ => {
            error!("error reading: {:?}", e);
            self.connection.set_error()?;
            return Err(e);
          }
        }
      }
    }
    if self.can_parse() {
      if let Ok(sz) = self.parse() {
        self.receive_buffer.consume(sz);
      }
    }
    if self.wants_to_read() || self.wants_to_write() || self.can_parse() {
      self.set_readiness.set_readiness(Ready::readable()).map_err(ErrorKind::IOError)?;
    }
    Ok(())
  }

  fn run(&mut self, events: &mut Events) -> Result<(), Error> {
    self.ensure_setup()?;
    self.poll.poll(events, None).map_err(ErrorKind::IOError)?;
    for event in events.iter() {
      match event.token() {
        SOCKET    => {
          if event.readiness().is_readable() {
            self.can_read = true;
          }
          if event.readiness().is_writable() {
            self.can_write = true;
          }
        },
        DATA      => self.has_data = true,
        HEARTBEAT => self.send_heartbeat = true,
        _         => {},
      }
    }
    self.do_run()
  }

  fn can_parse(&self) -> bool {
    self.receive_buffer.available_data() > 0
  }

  fn write_to_stream(&mut self) -> Result<(), Error> {
    let sz = self.serialize()?;
    self.send_buffer.fill(sz);

    self.socket.write(&self.send_buffer.data()).map(|sz| {
      trace!("wrote {} bytes", sz);
      self.send_buffer.consume(sz);
    }).map_err(|e| ErrorKind::IOError(e).into())
  }

  fn read_from_stream(&mut self) -> Result<(), Error> {
    let state = self.connection.status.state();
    if state == ConnectionState::Initial || state == ConnectionState::Closed || state == ConnectionState::Error {
      self.connection.set_error()?;
      Err(ErrorKind::InvalidConnectionState(state).into())
    } else {
      self.socket.read(&mut self.receive_buffer.space()).map(|sz| {
        trace!("read {} bytes", sz);
        self.receive_buffer.fill(sz);
      }).map_err(|e| ErrorKind::IOError(e).into())
    }
  }

  fn serialize(&mut self) -> Result<usize, Error> {
    if let Some((send_id, next_msg)) = self.connection.next_frame() {
      trace!("will write to buffer: {:?}", next_msg);
      match gen_frame(self.send_buffer.space(), &next_msg).map(|tup| tup.0) {
        Ok(sz) => {
          self.connection.mark_sent(send_id);
          Ok(sz)
        },
        Err(e) => {
          error!("error generating frame: {:?}", e);
          self.connection.set_error()?;
          match e {
            GenError::BufferTooSmall(_) => {
              // Requeue msg
              self.connection.requeue_frame(next_msg)?;
              self.send_buffer.shift();
              Err(ErrorKind::SendBufferTooSmall.into())
            },
            GenError::InvalidOffset | GenError::CustomError(_) | GenError::NotYetImplemented => {
              Err(ErrorKind::SerialisationError(e).into())
            }
          }
        }
      }
    } else {
      Err(ErrorKind::NoNewMessage.into())
    }
  }

  fn parse(&self) -> Result<usize, Error> {
    match parse_frame(self.receive_buffer.data()) {
      Ok((i, f)) => {
        let consumed = self.receive_buffer.data().offset(i);

        if let Err(e) = self.connection.handle_frame(f) {
          self.connection.set_error()?;
          Err(e)
        } else {
          Ok(consumed)
        }
      },
      Err(e) => {
        if e.is_incomplete() {
          Ok(0)
        } else {
          self.connection.set_error()?;
          Err(ErrorKind::ParsingError(format!("{:?}", e)).into())
        }
      }
    }
  }
}
