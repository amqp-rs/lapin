use crate::{
    buffer::Buffer, channels::Channels, connection_status::ConnectionState, frames::Frames,
    internal_rpc::InternalRPC, thread::ThreadHandle, waker::Waker, Configuration, ConnectionStatus,
    Error, PromiseResolver, Result,
};
use amq_protocol::frame::{gen_frame, parse_frame, AMQPFrame, GenError};
use log::{error, trace};
use mio::{event::Source, Events, Interest, Poll, Token, Waker as MioWaker};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    sync::Arc,
    thread::Builder as ThreadBuilder,
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
    connection_status: ConnectionStatus,
    configuration: Configuration,
    channels: Channels,
    internal_rpc: InternalRPC,
    frames: Frames,
    connection_io_loop_handle: ThreadHandle,
    waker: Waker,
    socket: T,
    status: Status,
    poll: Poll,
    frame_size: usize,
    receive_buffer: Buffer,
    send_buffer: Buffer,
    can_write: bool,
    can_read: bool,
    poll_timeout: Option<Duration>,
    serialized_frames: VecDeque<(u64, Option<PromiseResolver<()>>)>,
    last_write: Instant,
}

impl<T: Source + Read + Write + Send + 'static> IoLoop<T> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        connection_status: ConnectionStatus,
        configuration: Configuration,
        channels: Channels,
        internal_rpc: InternalRPC,
        frames: Frames,
        connection_io_loop_handle: ThreadHandle,
        waker: Waker,
        socket: T,
        poll: Option<(Poll, Token)>,
    ) -> Result<Self> {
        let (poll, registered) = poll
            .map(|t| Ok((t.0, true)))
            .unwrap_or_else(|| Poll::new().map(|poll| (poll, false)))?;
        waker.set_waker(MioWaker::new(poll.registry(), WAKER)?);
        let frame_size = std::cmp::max(8192, configuration.frame_max() as usize);
        let mut inner = Self {
            connection_status,
            configuration,
            channels,
            internal_rpc,
            frames,
            connection_io_loop_handle,
            waker,
            socket,
            status: Status::Initial,
            poll,
            frame_size,
            receive_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size),
            send_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size),
            can_write: false,
            can_read: false,
            poll_timeout: None,
            serialized_frames: VecDeque::default(),
            last_write: Instant::now(),
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

    fn ensure_setup(&mut self) -> Result<()> {
        if self.status != Status::Setup && self.connection_status.connected() {
            let frame_max = self.configuration.frame_max() as usize;
            self.frame_size = std::cmp::max(self.frame_size, frame_max);
            self.receive_buffer.grow(FRAMES_STORAGE * self.frame_size);
            self.send_buffer.grow(FRAMES_STORAGE * self.frame_size);
            let heartbeat = self.configuration.heartbeat();
            if heartbeat != 0 {
                let heartbeat = Duration::from_millis(u64::from(heartbeat) * 500); // * 1000 (ms) / 2 (half the negociated timeout)
                self.poll_timeout = Some(heartbeat);
            }
            self.status = Status::Setup;
        }
        Ok(())
    }

    fn has_data(&self) -> bool {
        self.frames.has_pending()
            || self.send_buffer.available_data() > 0
            || !self.serialized_frames.is_empty()
    }

    fn can_write(&self) -> bool {
        self.can_write && self.has_data() && !self.connection_status.blocked()
    }

    fn can_read(&self) -> bool {
        self.can_read
    }

    fn can_parse(&self) -> bool {
        self.receive_buffer.available_data() > 0
    }

    fn should_continue(&self) -> bool {
        (self.status == Status::Initial
            || self.connection_status.connected()
            || self.connection_status.closing())
            && self.status != Status::Stop
            && !self.connection_status.errored()
    }

    pub fn start(mut self) -> Result<()> {
        let waker = self.waker.clone();
        self.connection_io_loop_handle.clone().register(
            ThreadBuilder::new()
                .name("io_loop".to_owned())
                .spawn(move || {
                    let mut events = Events::with_capacity(1024);
                    while self.should_continue() {
                        if let Err(err) = self.run(&mut events) {
                            self.cancel_serialized_frames(err)?;
                        }
                    }
                    Ok(())
                })?,
        );
        waker.wake()
    }

    fn cancel_serialized_frames(&mut self, error: Error) -> Result<()> {
        for (_, resolver) in std::mem::take(&mut self.serialized_frames) {
            if let Some(resolver) = resolver {
                resolver.swear(Err(error.clone()));
            }
        }
        Err(error)
    }

    fn poll_timeout(&self) -> Option<Duration> {
        self.poll_timeout.map(|timeout| {
            timeout
                .checked_sub(self.last_write.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0))
        })
    }

    fn poll(&mut self, events: &mut Events) -> Result<()> {
        trace!("io_loop poll");
        self.poll.poll(events, self.poll_timeout())?;
        trace!("io_loop poll done");
        for event in events.iter() {
            if event.token() == SOCKET {
                trace!("Got mio event for socket: {:?}", event);
                if event.is_read_closed() || event.is_write_closed() {
                    self.critical_error(io::Error::from(io::ErrorKind::ConnectionReset).into())?;
                }
                if event.is_error() {
                    self.critical_error(io::Error::from(io::ErrorKind::ConnectionAborted).into())?;
                }
                // Due to a bug in epoll/mio, it doesn't seem like we can trust this, it's sometimes missing when it should be there
                /*
                if event.is_readable() {
                    self.can_read = true;
                }
                */
                self.can_read = true;
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
        let res = self.do_run();
        self.internal_rpc.poll(&self.channels).and(res)
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
            self.internal_rpc.poll(&self.channels)?;
            if self.connection_status.closed() {
                self.status = Status::Stop;
            }
            if self.should_continue() {
                self.read()?;
            }
            self.parse()?;
            self.internal_rpc.poll(&self.channels)?;
            if self.should_heartbeat() {
                self.channels.send_heartbeat()?;
                // Update last_write so that if we cannot write yet to the socket, we don't enqueue countless heartbeats
                self.last_write = Instant::now();
            }
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

    fn should_heartbeat(&self) -> bool {
        if let Some(heartbeat_timeout) = self.poll_timeout {
            self.last_write.elapsed() > heartbeat_timeout
        } else {
            false
        }
    }

    fn stop_looping(&self) -> bool {
        !self.can_read()
            || !self.can_write()
            || self.status == Status::Stop
            || self.connection_status.errored()
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
        if let Some(resolver) = self.connection_status.connection_resolver() {
            resolver.swear(Err(error.clone()));
        }
        self.status = Status::Stop;
        self.channels.set_connection_error(error.clone())?;
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
        }
        Ok(())
    }

    fn send_continue(&mut self) -> Result<()> {
        self.waker.wake()
    }

    fn write_to_stream(&mut self) -> Result<()> {
        self.serialize()?;

        let sz = self.send_buffer.write_to(&mut self.socket)?;

        if sz > 0 {
            self.last_write = Instant::now();

            trace!("wrote {} bytes", sz);
            self.send_buffer.consume(sz);

            let mut written = sz as u64;
            while written > 0 {
                if let Some((to_write, resolver)) = self.serialized_frames.pop_front() {
                    if written < to_write {
                        self.serialized_frames
                            .push_front((to_write - written, resolver));
                        trace!("{} to write to complete this frame", to_write - written);
                        written = 0;
                    } else {
                        if let Some(resolver) = resolver {
                            resolver.swear(Ok(()));
                        }
                        written -= to_write;
                    }
                } else {
                    error!(
                        "We've written {} but didn't expect to write anything",
                        written
                    );
                    break;
                }
            }

            if self.send_buffer.available_data() > 0 {
                // We didn't write all the data yet
                trace!("Still {} to write", self.send_buffer.available_data());
                self.send_continue()?;
            } else {
                self.socket.flush()?;
            }
        } else {
            error!("Socket was writable but we wrote 0, marking as wouldblock");
            self.can_write = false;
        }
        Ok(())
    }

    fn read_from_stream(&mut self) -> Result<()> {
        match self.connection_status.state() {
            ConnectionState::Closed => Ok(()),
            ConnectionState::Error => Err(Error::InvalidConnectionState(ConnectionState::Error)),
            _ => {
                self.receive_buffer.read_from(&mut self.socket).map(|sz| {
                    trace!("read {} bytes", sz);
                    self.receive_buffer.fill(sz);
                })?;
                Ok(())
            }
        }
    }

    fn serialize(&mut self) -> Result<()> {
        if let Some((next_msg, resolver)) = self.frames.pop(self.channels.flow()) {
            trace!("will write to buffer: {}", next_msg);
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
                            self.frames.retry((next_msg, resolver));
                            self.send_continue()
                        }
                        e => {
                            error!("error generating frame: {:?}", e);
                            let error = Error::SerialisationError(Arc::new(e));
                            self.channels.set_connection_error(error.clone())?;
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
                self.channels.handle_frame(frame)?;
            }
        }
        Ok(())
    }

    fn do_parse(&mut self) -> Result<Option<AMQPFrame>> {
        match parse_frame(self.receive_buffer.parsing_context()) {
            Ok((i, f)) => {
                let consumed = self.receive_buffer.offset(i);
                self.receive_buffer.consume(consumed);
                Ok(Some(f))
            }
            Err(e) => {
                if e.is_incomplete() {
                    Ok(None)
                } else {
                    error!("parse error: {:?}", e);
                    let error = Error::ParsingError(e);
                    self.channels.set_connection_error(error.clone())?;
                    Err(error)
                }
            }
        }
    }
}
