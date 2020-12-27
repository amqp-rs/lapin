use crate::{
    buffer::Buffer,
    channels::Channels,
    connection_status::ConnectionState,
    frames::Frames,
    heartbeat::Heartbeat,
    internal_rpc::InternalRPCHandle,
    protocol::{self, AMQPError, AMQPHardError},
    socket_state::{SocketEvent, SocketState},
    thread::ThreadHandle,
    types::FrameSize,
    Configuration, ConnectionStatus, Error, PromiseResolver, Result,
};
use amq_protocol::frame::{gen_frame, parse_frame, AMQPFrame, GenError};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
    thread::Builder as ThreadBuilder,
    time::Duration,
};
use reactor_trait::AsyncIOHandle;
use tracing::{error, trace};

const FRAMES_STORAGE: usize = 32;

#[derive(Debug, PartialEq)]
enum Status {
    Initial,
    Connected,
    Stop,
}

pub struct IoLoop {
    connection_status: ConnectionStatus,
    configuration: Configuration,
    channels: Channels,
    internal_rpc: InternalRPCHandle,
    frames: Frames,
    heartbeat: Heartbeat,
    socket_state: SocketState,
    connection_io_loop_handle: ThreadHandle,
    stream: Pin<Box<dyn AsyncIOHandle + Send>>,
    status: Status,
    frame_size: FrameSize,
    receive_buffer: Buffer,
    send_buffer: Buffer,
    serialized_frames: VecDeque<(FrameSize, Option<PromiseResolver<()>>)>,
}

impl IoLoop {
    pub(crate) async fn new(
        connection_status: ConnectionStatus,
        configuration: Configuration,
        channels: Channels,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        socket_state: SocketState,
        connection_io_loop_handle: ThreadHandle,
        stream: Pin<Box<dyn AsyncIOHandle + Send>>,
        heartbeat: Heartbeat,
    ) -> Result<Self> {
        let frame_size = std::cmp::max(
            protocol::constants::FRAME_MIN_SIZE,
            configuration.frame_max(),
        );

        Ok(Self {
            connection_status,
            configuration,
            channels,
            internal_rpc,
            frames,
            heartbeat,
            socket_state,
            connection_io_loop_handle,
            stream,
            status: Status::Initial,
            frame_size,
            receive_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size as usize),
            send_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size as usize),
            serialized_frames: VecDeque::default(),
        })
    }

    fn readable_waker(&self) -> Waker {
        let socket_state_handle = self.socket_state.handle();
        waker_fn::waker_fn(move || socket_state_handle.send(SocketEvent::Readable))
    }

    fn writable_waker(&self) -> Waker {
        let socket_state_handle = self.socket_state.handle();
        waker_fn::waker_fn(move || socket_state_handle.send(SocketEvent::Writable))
    }

    fn finish_setup(&mut self) -> Result<bool> {
        if self.connection_status.connected() {
            let frame_max = self.configuration.frame_max();
            self.frame_size = std::cmp::max(self.frame_size, frame_max);
            self.receive_buffer
                .grow(FRAMES_STORAGE * self.frame_size as usize);
            self.send_buffer
                .grow(FRAMES_STORAGE * self.frame_size as usize);
            let heartbeat = self.configuration.heartbeat();
            if heartbeat != 0 {
                let heartbeat = Duration::from_millis(u64::from(heartbeat) * 500); // * 1000 (ms) / 2 (half the negotiated timeout)
                self.heartbeat.set_timeout(heartbeat);
                self.heartbeat.start();
            }
            self.status = Status::Connected;
        }
        Ok(true)
    }

    fn ensure_setup(&mut self) -> Result<bool> {
        match self.status {
            Status::Initial => self.finish_setup(),
            Status::Connected => Ok(true),
            Status::Stop => Ok(false),
        }
    }

    fn has_data(&self) -> bool {
        self.frames.has_pending()
            || self.send_buffer.available_data() > 0
            || !self.serialized_frames.is_empty()
    }

    fn can_write(&mut self) -> bool {
        self.socket_state.writable() && self.has_data() && !self.connection_status.blocked()
    }

    fn can_read(&mut self) -> bool {
        self.socket_state.readable() && self.receive_buffer.available_space() > 0
    }

    fn can_parse(&self) -> bool {
        self.receive_buffer.available_data() > 0
    }

    fn should_continue(&self) -> bool {
        (self.status != Status::Connected
            || self.connection_status.connected()
            || self.connection_status.closing())
            && self.status != Status::Stop
            && !self.connection_status.errored()
    }

    pub fn start(mut self) -> Result<()> {
        let waker = self.socket_state.handle();
        let handle = self.connection_io_loop_handle.clone();
        handle.register(
            ThreadBuilder::new()
                .name("lapin-io-loop".to_owned())
                .spawn(move || {
                    let readable_waker = self.readable_waker();
                    let mut readable_context = Context::from_waker(&readable_waker);
                    let writable_waker = self.writable_waker();
                    let mut writable_context = Context::from_waker(&writable_waker);
                    let noop_waker = waker_fn::waker_fn(|| {});
                    let mut noop_context = Context::from_waker(&noop_waker);
                    while self.should_continue() {
                        if let Err(err) = self.run(
                            &mut readable_context,
                            &mut writable_context,
                            &mut noop_context,
                        ) {
                            self.critical_error(err)?;
                        }
                    }
                    self.internal_rpc.stop();
                    self.heartbeat.cancel();
                    Ok(())
                })?,
        );
        waker.wake();
        Ok(())
    }

    fn poll_socket_events(&mut self) {
        self.socket_state.poll_events();
    }

    fn check_connection_state(&mut self) {
        if self.connection_status.closed() {
            self.status = Status::Stop;
        }
    }

    fn run(
        &mut self,
        readable_context: &mut Context<'_>,
        writable_context: &mut Context<'_>,
        noop_context: &mut Context<'_>,
    ) -> Result<()> {
        trace!("io_loop run");
        self.poll_socket_events();
        if !self.ensure_setup()? {
            return Ok(());
        }
        self.check_connection_state();
        trace!(
            can_read=%self.socket_state.readable(),
            can_write=%self.socket_state.writable(),
            has_data=%self.has_data(),
            "io_loop do_run",
        );
        if !self.can_read() && !self.can_write() && self.should_continue() {
            self.socket_state.wait();
        }
        self.poll_socket_events();
        self.attempt_flush(noop_context)?;
        self.write(writable_context)?;
        self.check_connection_state();
        if self.should_continue() {
            self.read(readable_context)?;
        }
        self.handle_frames()?;
        self.check_connection_state();
        trace!(
            can_read=%self.socket_state.readable(),
            can_write=%self.socket_state.writable(),
            has_data=%self.has_data(),
            status=?self.status,
            "io_loop do_run done",
        );
        Ok(())
    }

    fn critical_error(&mut self, error: Error) -> Result<()> {
        if let Some(resolver) = self.connection_status.connection_resolver() {
            resolver.swear(Err(error.clone()));
        }
        self.status = Status::Stop;
        self.channels.set_connection_error(error.clone());
        for (_, resolver) in std::mem::take(&mut self.serialized_frames) {
            if let Some(resolver) = resolver {
                resolver.swear(Err(error.clone()));
            }
        }
        Err(error)
    }

    fn attempt_flush(&mut self, noop_context: &mut Context<'_>) -> Result<()> {
        let res = self.flush(noop_context);
        self.handle_io_result(res)
    }

    fn handle_io_result(&mut self, result: Result<()>) -> Result<()> {
        if let Err(e) = self.socket_state.handle_io_result(result) {
            error!(error=?e, "error doing IO");
            self.critical_error(e)?;
        }
        Ok(())
    }

    fn flush(&mut self, writable_context: &mut Context<'_>) -> Result<()> {
        let res = self.stream.as_mut().poll_flush(writable_context)?;
        self.socket_state.handle_write_poll(res);
        Ok(())
    }

    fn write(&mut self, writable_context: &mut Context<'_>) -> Result<()> {
        while self.can_write() {
            let res = self.write_to_stream(writable_context);
            self.handle_io_result(res)?;
        }
        Ok(())
    }

    fn read(&mut self, readable_context: &mut Context<'_>) -> Result<()> {
        while self.can_read() {
            let res = self.read_from_stream(readable_context);
            self.handle_io_result(res)?;
        }
        Ok(())
    }

    fn write_to_stream(&mut self, writable_context: &mut Context<'_>) -> Result<()> {
        self.flush(writable_context)?;
        self.serialize()?;

        let res = self
            .send_buffer
            .poll_write_to(writable_context, Pin::new(&mut self.stream))?;

        if let Some(sz) = self.socket_state.handle_write_poll(res) {
            if sz > 0 {
                self.heartbeat.update_last_write();

                trace!("wrote {} bytes", sz);
                self.send_buffer.consume(sz);

                let mut written = sz as FrameSize;
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
                }

                self.flush(writable_context)?;
            } else {
                error!("Socket was writable but we wrote 0, marking as wouldblock");
                self.socket_state.handle_write_poll::<()>(Poll::Pending);
            }
        }
        Ok(())
    }

    fn read_from_stream(&mut self, readable_context: &mut Context<'_>) -> Result<()> {
        match self.connection_status.state() {
            ConnectionState::Closed => Ok(()),
            ConnectionState::Error => Err(Error::InvalidConnectionState(ConnectionState::Error)),
            _ => {
                let res = self
                    .receive_buffer
                    .poll_read_from(readable_context, Pin::new(&mut self.stream))?;

                if let Some(sz) = self.socket_state.handle_read_poll(res) {
                    if sz > 0 {
                        trace!("read {} bytes", sz);
                        self.receive_buffer.fill(sz);
                    } else {
                        error!("Socket was readable but we read 0, marking as wouldblock");
                        self.socket_state.handle_read_poll(Poll::Pending);
                    }
                }
                Ok(())
            }
        }
    }

    fn serialize(&mut self) -> Result<()> {
        while let Some((next_msg, resolver)) = self.frames.pop(self.channels.flow()) {
            trace!(%next_msg, "will write to buffer");
            let checkpoint = self.send_buffer.checkpoint();
            let res = gen_frame(&next_msg)((&mut self.send_buffer).into());
            match res.map(|w| w.into_inner().1) {
                Ok(sz) => self
                    .serialized_frames
                    .push_back((sz as FrameSize, resolver)),
                Err(e) => {
                    self.send_buffer.rollback(checkpoint);
                    match e {
                        GenError::BufferTooSmall(_) => {
                            // Requeue msg
                            self.frames.retry((next_msg, resolver));
                            break;
                        }
                        e => {
                            error!(error=?e, "error generating frame");
                            self.critical_error(Error::SerialisationError(Arc::new(e)))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_frames(&mut self) -> Result<()> {
        while self.can_parse() {
            if let Some(frame) = self.parse()? {
                self.channels.handle_frame(frame)?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn parse(&mut self) -> Result<Option<AMQPFrame>> {
        match parse_frame(self.receive_buffer.parsing_context()) {
            Ok((i, f)) => {
                let consumed = self.receive_buffer.offset(i);
                let frame_max = self.configuration.frame_max() as usize;
                if frame_max > 0 && consumed > frame_max {
                    error!(bytes = consumed, "received large frame");
                    let error = AMQPError::new(
                        AMQPHardError::FRAMEERROR.into(),
                        format!("frame too large: {} bytes", consumed).into(),
                    );
                    self.internal_rpc.close_connection(
                        error.get_id(),
                        error.get_message().to_string(),
                        0,
                        0,
                    );
                    self.critical_error(Error::ProtocolError(error))?;
                }
                self.receive_buffer.consume(consumed);
                Ok(Some(f))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    error!(error=?e, "parse error");
                    self.critical_error(Error::ParsingError(e))?;
                }
                Ok(None)
            }
        }
    }
}
