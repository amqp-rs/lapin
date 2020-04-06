use crate::{
    buffer::Buffer, connection::Connection, connection_status::ConnectionState, Error,
    PromiseResolver, Result,
};
use amq_protocol::frame::{gen_frame, parse_frame, AMQPFrame, GenError};
use log::{debug, error, trace};
use mio::{event::Source, Events, Interest, Poll, Token, Waker};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    sync::Arc,
    thread::{self, Builder as ThreadBuilder, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) const SOCKET: Token = Token(1);
const WAKER: Token = Token(2);

const FRAMES_STORAGE: usize = 32;

#[derive(Debug, PartialEq)]
enum Status {
    Initial,
    Setup,
    Stop,
}

pub struct IoLoop<T> {
    connection: Connection,
    socket: T,
    status: Status,
    poll: Poll,
    waker: Arc<Waker>,
    hb_handle: Option<JoinHandle<()>>,
    frame_size: usize,
    receive_buffer: Buffer,
    send_buffer: Buffer,
    can_write: bool,
    can_read: bool,
    poll_timeout: Option<Duration>,
    serialized_frames: VecDeque<(u64, Option<PromiseResolver<()>>)>,
}

impl<T: Source + Read + Write + Send + 'static> IoLoop<T> {
    pub(crate) fn new(
        connection: Connection,
        socket: T,
        poll: Option<(Poll, Token)>,
    ) -> Result<Self> {
        let (poll, registered) = poll
            .map(|t| Ok((t.0, true)))
            .unwrap_or_else(|| Poll::new().map(|poll| (poll, false)))?;
        let frame_size = std::cmp::max(8192, connection.configuration().frame_max() as usize);
        let waker = Arc::new(Waker::new(poll.registry(), WAKER)?);
        let mut inner = Self {
            connection,
            socket,
            status: Status::Initial,
            poll,
            waker,
            hb_handle: None,
            frame_size,
            receive_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size),
            send_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size),
            can_write: false,
            can_read: false,
            poll_timeout: None,
            serialized_frames: VecDeque::default(),
        };
        if registered {
            inner.poll.registry().reregister(
                &mut inner.socket,
                SOCKET,
                Interest::READABLE | Interest::WRITABLE,
            )?;
        } else {
            inner.poll.registry().register(
                &mut inner.socket,
                SOCKET,
                Interest::READABLE | Interest::WRITABLE,
            )?;
        }
        Ok(inner)
    }

    fn start_heartbeat(&mut self, interval: Duration) -> Result<()> {
        let connection = self.connection.clone();
        let hb_handle = ThreadBuilder::new()
            .name("heartbeat".to_owned())
            .spawn(move || {
                while connection.status().connected() {
                    let start = Instant::now();
                    let mut remaining = interval;

                    loop {
                        thread::park_timeout(remaining);
                        let elapsed = start.elapsed();
                        if elapsed >= remaining {
                            break;
                        }
                        remaining -= interval - elapsed;
                    }

                    debug!("send heartbeat");
                    if let Err(err) = connection.send_heartbeat() {
                        error!("Error while sending heartbeat: {}", err);
                    }
                }
            })?;
        self.hb_handle = Some(hb_handle);
        Ok(())
    }

    fn ensure_setup(&mut self) -> Result<()> {
        if self.status != Status::Setup && self.connection.status().connected() {
            let frame_max = self.connection.configuration().frame_max() as usize;
            self.frame_size = std::cmp::max(self.frame_size, frame_max);
            self.receive_buffer.grow(FRAMES_STORAGE * self.frame_size);
            self.send_buffer.grow(FRAMES_STORAGE * self.frame_size);
            let heartbeat = self.connection.configuration().heartbeat();
            if heartbeat != 0 {
                trace!("io_loop: start heartbeat");
                let heartbeat = Duration::from_secs(u64::from(heartbeat / 2));
                self.start_heartbeat(heartbeat)?;
                self.poll_timeout = Some(heartbeat);
                trace!("io_loop: heartbeat started");
            }
            self.status = Status::Setup;
        }
        Ok(())
    }

    fn has_data(&self) -> bool {
        self.connection.has_pending_frames()
            || self.send_buffer.available_data() > 0
            || !self.serialized_frames.is_empty()
    }

    fn can_write(&self) -> bool {
        self.can_write && self.has_data() && !self.connection.status().blocked()
    }

    fn can_read(&self) -> bool {
        self.can_read
    }

    fn can_parse(&self) -> bool {
        self.receive_buffer.available_data() > 0
    }

    fn should_continue(&self) -> bool {
        let connection_status = self.connection.status();
        (self.status == Status::Initial
            || connection_status.connected()
            || connection_status.closing())
            && self.status != Status::Stop
            && !connection_status.errored()
    }

    pub fn start(mut self) -> Result<()> {
        let waker = self.waker.clone();
        self.connection.clone().set_io_loop(
            ThreadBuilder::new()
                .name("io_loop".to_owned())
                .spawn(move || {
                    let mut events = Events::with_capacity(1024);
                    while self.should_continue() {
                        self.run(&mut events)?;
                    }
                    if let Some(hb_handle) = self.hb_handle.take() {
                        hb_handle.thread().unpark();
                        hb_handle.join().expect("heartbeat loop failed");
                    }
                    Ok(())
                })?,
            waker,
        )
    }

    fn poll(&mut self, events: &mut Events) -> Result<()> {
        trace!("io_loop poll");
        self.poll.poll(events, self.poll_timeout)?;
        trace!("io_loop poll done");
        for event in events.iter() {
            if event.token() == SOCKET {
                if event.is_readable() {
                    self.can_read = true;
                }
                if event.is_writable() {
                    self.can_write = true;
                }
            }
        }
        Ok(())
    }

    fn run(&mut self, events: &mut Events) -> Result<()> {
        trace!("io_loop run");
        self.ensure_setup()?;
        self.poll(events)?;
        self.do_run()
    }

    fn do_run(&mut self) -> Result<()> {
        trace!(
            "io_loop do_run; can_read={}, can_write={}, has_data={}",
            self.can_read,
            self.can_write,
            self.has_data()
        );
        loop {
            self.write()?;
            if self.connection.status().closed() {
                self.status = Status::Stop;
            }
            if self.should_continue() {
                self.read()?;
            }
            self.parse()?;
            self.connection.poll_internal_promises()?;
            if self.stop_looping() {
                self.maybe_continue()?;
                break;
            }
        }
        trace!(
            "io_loop do_run done; can_read={}, can_write={}, has_data={}, status={:?}",
            self.can_read,
            self.can_write,
            self.has_data(),
            self.status
        );
        Ok(())
    }

    fn stop_looping(&self) -> bool {
        !self.can_read()
            || !self.can_write()
            || self.status == Status::Stop
            || self.connection.status().errored()
    }

    fn has_pending_operations(&self) -> bool {
        self.status != Status::Stop && (self.can_read() || self.can_parse() || self.can_write())
    }

    fn maybe_continue(&mut self) -> Result<()> {
        if self.has_pending_operations() {
            trace!(
                "io_loop send continue; can_read={}, can_write={}, has_data={}",
                self.can_read,
                self.can_write,
                self.has_data()
            );
            self.send_continue()?;
        }
        Ok(())
    }

    fn critical_error(&mut self, error: Error) -> Result<()> {
        if let ConnectionState::SentProtocolHeader(resolver, ..) = self.connection.status().state()
        {
            resolver.swear(Err(error.clone()));
            self.status = Status::Stop;
        }
        self.connection.set_error(error.clone())?;
        Err(error)
    }

    fn write(&mut self) -> Result<()> {
        if self.can_write() {
            if let Err(e) = self.write_to_stream() {
                if e.wouldblock() {
                    self.can_write = false
                } else {
                    error!("error writing: {:?}", e);
                    self.critical_error(e)?;
                }
            }
        }
        Ok(())
    }

    fn read(&mut self) -> Result<()> {
        if self.can_read() {
            if let Err(e) = self.read_from_stream() {
                if e.wouldblock() {
                    self.can_read = false
                } else {
                    error!("error reading: {:?}", e);
                    self.critical_error(e)?;
                }
            }
            self.receive_buffer.shift_unless_available(self.frame_size);
        }
        Ok(())
    }

    fn send_continue(&mut self) -> Result<()> {
        self.waker.wake()?;
        Ok(())
    }

    fn write_to_stream(&mut self) -> Result<()> {
        self.serialize()?;

        let sz = self.send_buffer.write_to(&mut self.socket)?;

        trace!("wrote {} bytes", sz);
        self.send_buffer.consume(sz, true);

        let mut written = sz as u64;
        while written > 0 {
            if let Some((to_write, resolver)) = self.serialized_frames.pop_front() {
                if written < to_write {
                    self.serialized_frames
                        .push_front((to_write - written, resolver));
                    written = 0;
                } else {
                    if let Some(resolver) = resolver {
                        resolver.swear(Ok(()));
                    }
                    written -= to_write;
                }
            } else {
                break;
            }
        }

        if sz > 0 && self.send_buffer.available_data() > 0 {
            // We didn't write all the data yet
            self.send_continue()?;
        }
        Ok(())
    }

    fn read_from_stream(&mut self) -> Result<()> {
        match self.connection.status().state() {
            ConnectionState::Closed => Ok(()),
            ConnectionState::Error => Err(Error::InvalidConnectionState(ConnectionState::Error)),
            _ => {
                self.receive_buffer
                    .read_from(&mut self.socket, false)
                    .map(|sz| {
                        trace!("read {} bytes", sz);
                        self.receive_buffer.fill(sz, false);
                    })?;
                Ok(())
            }
        }
    }

    fn serialize(&mut self) -> Result<()> {
        if let Some((next_msg, resolver)) = self.connection.next_frame() {
            trace!("will write to buffer: {:?}", next_msg);
            let checkpoint = self.send_buffer.checkpoint();
            let res = gen_frame(&next_msg)((&mut self.send_buffer).into());
            match res.map(|w| w.into_inner().1) {
                Ok(sz) => {
                    self.serialized_frames.push_back((sz, resolver));
                    Ok(())
                }
                Err(e) => {
                    self.send_buffer.rollback(checkpoint);
                    match e {
                        GenError::BufferTooSmall(_) => {
                            // Requeue msg
                            self.connection.requeue_frame((next_msg, resolver))?;
                            Ok(())
                        }
                        e => {
                            error!("error generating frame: {:?}", e);
                            let error = Error::SerialisationError(Arc::new(e));
                            self.connection.set_error(error.clone())?;
                            Err(error)
                        }
                    }
                }
            }
        } else {
            Ok(())
        }
    }

    fn parse(&mut self) -> Result<()> {
        if self.can_parse() {
            if let Some(frame) = self.do_parse()? {
                self.connection.handle_frame(frame)?;
            }
        }
        Ok(())
    }

    fn do_parse(&mut self) -> Result<Option<AMQPFrame>> {
        match parse_frame(self.receive_buffer.data()) {
            Ok((i, f)) => {
                let consumed = self.receive_buffer.offset(i);
                self.receive_buffer.consume(consumed, false);
                Ok(Some(f))
            }
            Err(e) => {
                if e.is_incomplete() {
                    self.receive_buffer.shift();
                    Ok(None)
                } else {
                    error!("parse error: {:?}", e);
                    let error = Error::ParsingError(e);
                    self.connection.set_error(error.clone())?;
                    Err(error)
                }
            }
        }
    }
}
