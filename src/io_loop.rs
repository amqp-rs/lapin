use crate::{
    buffer::Buffer, connection::Connection, connection_status::ConnectionState, Error, Result,
};
use amq_protocol::frame::{gen_frame, parse_frame, AMQPFrame, GenError, Offset};
use log::{error, trace};
use mio::{Evented, Events, Poll, PollOpt, Ready, Registration, SetReadiness, Token};
use parking_lot::Mutex;
use std::{
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, Builder as ThreadBuilder, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) const SOCKET: Token = Token(1);
const DATA: Token = Token(2);
const CONTINUE: Token = Token(3);

const FRAMES_STORAGE: usize = 32;

type ThreadHandle = JoinHandle<Result<()>>;

#[derive(Clone, Debug)]
pub(crate) struct IoLoopHandle {
    handle: Arc<Mutex<Option<ThreadHandle>>>,
}

impl Default for IoLoopHandle {
    fn default() -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl IoLoopHandle {
    pub(crate) fn register(&self, handle: JoinHandle<Result<()>>) {
        *self.handle.lock() = Some(handle);
    }

    pub(crate) fn wait(&self) -> Result<()> {
        if let Some(handle) = self.handle.lock().take() {
            handle.join().expect("io loop")?
        }
        Ok(())
    }
}

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
    registration: Registration,
    set_readiness: SetReadiness,
    hb_handle: Option<JoinHandle<()>>,
    frame_size: usize,
    receive_buffer: Buffer,
    send_buffer: Buffer,
    can_write: bool,
    can_read: bool,
    has_data: bool,
    send_heartbeat: Arc<AtomicBool>,
    poll_timeout: Option<Duration>,
}

impl<T: Evented + Read + Write + Send + 'static> IoLoop<T> {
    pub(crate) fn new(
        connection: Connection,
        socket: T,
        poll: Option<(Poll, Token)>,
    ) -> Result<Self> {
        let (poll, registered) = poll.map(|t| Ok((t.0, true))).unwrap_or_else(|| {
            Poll::new()
                .map(|poll| (poll, false))
                .map_err(Arc::new)
                .map_err(Error::IOError)
        })?;
        let frame_size = std::cmp::max(8192, connection.configuration().frame_max() as usize);
        let (registration, set_readiness) = Registration::new2();
        let inner = Self {
            connection,
            socket,
            status: Status::Initial,
            poll,
            registration,
            set_readiness,
            hb_handle: None,
            frame_size,
            receive_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size),
            send_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size),
            can_write: false,
            can_read: false,
            has_data: false,
            send_heartbeat: Arc::new(AtomicBool::new(false)),
            poll_timeout: None,
        };
        if registered {
            inner
                .poll
                .reregister(&inner.socket, SOCKET, Ready::all(), PollOpt::edge())
                .map_err(Arc::new)
                .map_err(Error::IOError)?;
        } else {
            inner
                .poll
                .register(&inner.socket, SOCKET, Ready::all(), PollOpt::edge())
                .map_err(Arc::new)
                .map_err(Error::IOError)?;
        }
        inner
            .poll
            .register(&inner.connection, DATA, Ready::readable(), PollOpt::edge())
            .map_err(Arc::new)
            .map_err(Error::IOError)?;
        inner
            .poll
            .register(
                &inner.registration,
                CONTINUE,
                Ready::readable(),
                PollOpt::edge(),
            )
            .map_err(Arc::new)
            .map_err(Error::IOError)?;
        Ok(inner)
    }

    fn start_heartbeat(&mut self, interval: Duration) -> Result<()> {
        let connection = self.connection.clone();
        let send_hartbeat = self.send_heartbeat.clone();
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

                    send_hartbeat.store(true, Ordering::Relaxed);
                }
            })
            .map_err(Arc::new)
            .map_err(Error::IOError)?;
        self.hb_handle = Some(hb_handle);
        Ok(())
    }

    fn heartbeat(&mut self) -> Result<()> {
        if self.send_heartbeat.load(Ordering::Relaxed) {
            trace!("send heartbeat");
            self.connection.send_heartbeat()?;
            self.send_heartbeat.store(false, Ordering::Relaxed);
        }
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
                let heartbeat = Duration::from_secs(u64::from(heartbeat));
                self.start_heartbeat(heartbeat)?;
                self.poll_timeout = Some(heartbeat);
                trace!("io_loop: heartbeat started");
            }
            self.status = Status::Setup;
        }
        Ok(())
    }

    fn can_write(&self) -> bool {
        self.can_write && self.has_data && !self.connection.status().blocked()
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
                })
                .map_err(Arc::new)
                .map_err(Error::IOError)?,
        );
        Ok(())
    }

    fn poll(&mut self, events: &mut Events) -> Result<()> {
        trace!("io_loop poll");
        self.poll
            .poll(events, self.poll_timeout)
            .map_err(Arc::new)
            .map_err(Error::IOError)?;
        trace!("io_loop poll done");
        for event in events.iter() {
            match event.token() {
                SOCKET => {
                    if event.readiness().is_readable() {
                        self.can_read = true;
                    }
                    if event.readiness().is_writable() {
                        self.can_write = true;
                    }
                }
                DATA => self.has_data = true,
                _ => {}
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
            self.has_data
        );
        loop {
            self.heartbeat()?;
            self.write()?;
            if self.connection.status().closed() {
                self.status = Status::Stop;
            }
            if self.should_continue() {
                self.read()?;
            }
            self.parse()?;
            if self.stop_looping() {
                self.maybe_continue()?;
                break;
            }
        }
        trace!(
            "io_loop do_run done; can_read={}, can_write={}, has_data={}, status={:?}",
            self.can_read,
            self.can_write,
            self.has_data,
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
                self.has_data
            );
            self.send_continue()?;
        }
        Ok(())
    }

    fn write(&mut self) -> Result<()> {
        if self.can_write() {
            if let Err(e) = self.write_to_stream() {
                if e.wouldblock() {
                    self.can_write = false
                } else {
                    error!("error writing: {:?}", e);
                    if let ConnectionState::SentProtocolHeader(pinky, ..) =
                        self.connection.status().state()
                    {
                        pinky.swear(Err(Error::ConnectionRefused));
                        self.status = Status::Stop;
                    }
                    self.connection.set_error()?;
                    return Err(e);
                }
            }
            self.send_buffer.shift_unless_available(self.frame_size);
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
                    self.connection.set_error()?;
                    return Err(e);
                }
            }
            self.receive_buffer.shift_unless_available(self.frame_size);
        }
        Ok(())
    }

    fn send_continue(&mut self) -> Result<()> {
        self.set_readiness
            .set_readiness(Ready::readable())
            .map_err(Arc::new)
            .map_err(Error::IOError)
    }

    fn write_to_stream(&mut self) -> Result<()> {
        self.serialize()?;

        let sz = self
            .socket
            .write(&self.send_buffer.data())
            .map_err(Arc::new)
            .map_err(Error::IOError)?;

        trace!("wrote {} bytes", sz);
        self.send_buffer.consume(sz);
        if self.send_buffer.available_data() > 0 {
            // We didn't write all the data yet
            self.connection.set_readable()?;
        }
        Ok(())
    }

    fn read_from_stream(&mut self) -> Result<()> {
        match self.connection.status().state() {
            ConnectionState::Closed => Ok(()),
            ConnectionState::Error => Err(Error::InvalidConnectionState(ConnectionState::Error)),
            _ => self
                .socket
                .read(&mut self.receive_buffer.space())
                .map(|sz| {
                    trace!("read {} bytes", sz);
                    self.receive_buffer.fill(sz);
                })
                .map_err(Arc::new)
                .map_err(Error::IOError),
        }
    }

    fn serialize(&mut self) -> Result<()> {
        if let Some((send_id, next_msg)) = self.connection.next_frame() {
            trace!("will write to buffer: {:?}", next_msg);
            let checkpoint = self.send_buffer.checkpoint();
            let res = gen_frame(&next_msg)((&mut self.send_buffer).into());
            match res.map(|w| w.into_inner().1) {
                Ok(_) => {
                    self.connection.mark_sent(send_id);
                    Ok(())
                }
                Err(e) => {
                    self.send_buffer.rollback(checkpoint);
                    match e {
                        GenError::BufferTooSmall(_) => {
                            // Requeue msg
                            self.connection.requeue_frame(send_id, next_msg)?;
                            self.send_buffer.shift();
                            Ok(())
                        }
                        e => {
                            error!("error generating frame: {:?}", e);
                            self.connection.set_error()?;
                            Err(Error::SerialisationError(Arc::new(e)))
                        }
                    }
                }
            }
        } else {
            self.has_data = false;
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
                let consumed = self.receive_buffer.data().offset(i);
                self.receive_buffer.consume(consumed);
                Ok(Some(f))
            }
            Err(e) => {
                if e.is_incomplete() {
                    self.receive_buffer.shift();
                    Ok(None)
                } else {
                    error!("parse error: {:?}", e);
                    self.connection.set_error()?;
                    Err(Error::ParsingError(format!("{:?}", e)))
                }
            }
        }
    }
}
