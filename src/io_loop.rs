use crate::{
    AsyncTcpStream, Configuration, ConnectionState, ConnectionStatus, Error, ErrorKind, Promise,
    PromiseResolver, Result,
    buffer::Buffer,
    channels::Channels,
    frames::Frames,
    heartbeat::Heartbeat,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    protocol::{self, AMQPError, AMQPHardError},
    reactor::FullReactor,
    socket_state::SocketState,
    thread::JoinHandle,
    types::FrameSize,
    uri::AMQPUri,
};
use amq_protocol::frame::{AMQPFrame, GenError, gen_frame, parse_frame};
use executor_trait::FullExecutor;
use futures_io::{AsyncRead, AsyncWrite};
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    thread::Builder as ThreadBuilder,
};
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
    socket_state: SocketState,
    heartbeat: Heartbeat,
    connect: Arc<
        dyn (Fn(
                AMQPUri,
                Arc<dyn FullExecutor + Send + Sync + 'static>,
                Arc<dyn FullReactor + Send + Sync + 'static>,
            ) -> Box<dyn Future<Output = io::Result<AsyncTcpStream>> + Send>)
            + Send
            + Sync,
    >,
    uri: AMQPUri,
    status: Status,
    frame_size: FrameSize,
    receive_buffer: Buffer,
    send_buffer: Buffer,
    serialized_frames: VecDeque<(FrameSize, Option<PromiseResolver<()>>)>,
}

impl IoLoop {
    pub(crate) fn new(
        connection_status: ConnectionStatus,
        configuration: Configuration,
        channels: Channels,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        socket_state: SocketState,
        heartbeat: Heartbeat,
        connect: Arc<
            dyn (Fn(
                    AMQPUri,
                    Arc<dyn FullExecutor + Send + Sync + 'static>,
                    Arc<dyn FullReactor + Send + Sync + 'static>,
                ) -> Box<dyn Future<Output = io::Result<AsyncTcpStream>> + Send>)
                + Send
                + Sync,
        >,
        uri: AMQPUri,
    ) -> Self {
        let frame_size = std::cmp::max(
            protocol::constants::FRAME_MIN_SIZE,
            configuration.frame_max(),
        );

        Self {
            connection_status,
            configuration,
            channels,
            internal_rpc,
            frames,
            socket_state,
            heartbeat,
            connect,
            uri,
            status: Status::Initial,
            frame_size,
            receive_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size as usize),
            send_buffer: Buffer::with_capacity(FRAMES_STORAGE * frame_size as usize),
            serialized_frames: VecDeque::default(),
        }
    }

    fn reset(&mut self) {
        self.status = Status::Initial;
        self.receive_buffer.reset();
        self.send_buffer.reset();
    }

    fn finish_setup(&mut self) -> Result<bool> {
        if self.connection_status.connected() {
            let frame_max = self.configuration.frame_max();
            self.frame_size = std::cmp::max(self.frame_size, frame_max);
            self.receive_buffer
                .grow(FRAMES_STORAGE * self.frame_size as usize);
            self.send_buffer
                .grow(FRAMES_STORAGE * self.frame_size as usize);
            self.channels.start_heartbeat();
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

    fn reconnecting(&self) -> bool {
        self.connection_status.reconnecting()
    }

    fn should_continue(&self, connection_killswitch: &KillSwitch) -> bool {
        if self.connection_status.errored() {
            return false;
        }

        trace!(status=?self.status, connection_status=?self.connection_status.state(), internal_rpc_empty=%self.internal_rpc.is_empty(), frames_pending=%self.frames.has_pending(), conn_killed=%connection_killswitch.killed(), ser_frames_empty=%self.serialized_frames.is_empty(), "Should continue?");

        match self.status {
            Status::Initial => true,
            Status::Stop => false,
            Status::Connected => {
                self.connection_status.connected()
                    || self.connection_status.closing()
                    || !self.internal_rpc.is_empty()
                    || self.frames.has_pending()
                    || !connection_killswitch.killed()
                    || !self.serialized_frames.is_empty()
            }
        }
    }

    pub(crate) fn start(
        mut self,
        executor: Arc<dyn FullExecutor + Send + Sync + 'static>,
        reactor: Arc<dyn FullReactor + Send + Sync + 'static>,
    ) -> Result<JoinHandle> {
        let waker = self.socket_state.handle();
        let current_span = tracing::Span::current();
        let handle = ThreadBuilder::new()
            .name("lapin-io-loop".to_owned())
            .spawn(move || {
                let _enter = current_span.enter();
                let connection_killswitch = self.channels.connection_killswitch();
                let readable_waker = self.socket_state.readable_waker();
                let mut readable_context = Context::from_waker(&readable_waker);
                let writable_waker = self.socket_state.writable_waker();
                let mut writable_context = Context::from_waker(&writable_waker);
                let (mut stream, res) = loop {
                    let (promise, resolver) = Promise::new();
                    let connect = Box::into_pin((self.connect)(self.uri.clone(), executor.clone(), reactor.clone()));
                    self.internal_rpc.spawn_with_resolver(async move { Ok(connect.await?) }, resolver);
                    let mut stream = promise.wait().inspect_err(|err| {
                        trace!("Poison connection attempt");
                        self.connection_status.poison(err.clone());
                    })?;
                    let mut res = Ok(());

                    while self.should_continue(&connection_killswitch) {
                        if let Err(err) = self.run(Pin::new(&mut stream), &mut readable_context, &mut writable_context, &connection_killswitch) {
                            res = self.critical_error(&connection_killswitch, err);
                        }
                    }

                    let reconnect = self.reconnecting();

                    trace!(status=?self.status, connection_status=?self.connection_status.state(), "io_loop exiting for {}", if reconnect { "reconnection" } else { "shutdown" });
                    self.clear_serialized_frames(self.frames.poison().or_else(|| res.clone().err()).unwrap_or(
                        ErrorKind::InvalidConnectionState(ConnectionState::Closed).into(),
                    ));

                    if !reconnect {
                        break (stream, res);
                    }

                    self.reset();
                    self.socket_state.reset();
                    self.internal_rpc.start_channels_recovery();
                };

                trace!(status=?self.status, connection_status=?self.connection_status.state(), "io_loop entering exit/cleanup phase");
                let internal_rpc = self.internal_rpc.clone();
                if self.heartbeat.killswitch().killed() {
                    internal_rpc.spawn(std::future::poll_fn(move |cx| {
                        Pin::new(&mut stream)
                            .poll_close(cx)
                            .map(|res| res.map_err(From::from))
                    }));
                }
                internal_rpc.stop();
                res
            })?;
        waker.wake();
        Ok(handle)
    }

    fn stop(&mut self) {
        self.status = Status::Stop;
    }

    fn poll_socket_events(&mut self) {
        self.socket_state.poll_events();
    }

    fn check_connection_state(&mut self) {
        if self.connection_status.closed() {
            self.stop();
        }
    }

    fn run<T: AsyncRead + AsyncWrite + ?Sized>(
        &mut self,
        mut stream: Pin<&mut T>,
        readable_context: &mut Context<'_>,
        writable_context: &mut Context<'_>,
        connection_killswitch: &KillSwitch,
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
        if !self.can_read() && !self.can_write() && self.should_continue(connection_killswitch) {
            trace!("io_loop cannot do anything for now, waiting for socket events.");
            self.socket_state.wait();
        }
        self.poll_socket_events();
        self.attempt_flush(stream.as_mut(), writable_context, connection_killswitch)?;
        self.write(stream.as_mut(), writable_context, connection_killswitch)?;
        self.check_connection_state();
        if self.should_continue(connection_killswitch) {
            self.read(stream, readable_context, connection_killswitch)?;
        }
        self.handle_frames(connection_killswitch)?;
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

    fn critical_error(&mut self, connection_killswitch: &KillSwitch, error: Error) -> Result<()> {
        if error.is_io_error() {
            connection_killswitch.kill();
        }
        if self
            .channels
            .recovery_config()
            .can_recover_connection(&error)
        {
            self.internal_rpc.init_connection_recovery(error);
            return Ok(());
        }

        if let Some(resolver) = self.connection_status.connection_resolver() {
            resolver.reject(error.clone());
        }
        self.stop();
        self.channels.set_connection_error(error.clone());
        self.clear_serialized_frames(error.clone());
        Err(error)
    }

    fn clear_serialized_frames(&mut self, error: Error) {
        for (_, resolver) in std::mem::take(&mut self.serialized_frames) {
            if let Some(resolver) = resolver {
                trace!(
                    "We're quitting but had leftover frames, tag them as 'not sent' with current error"
                );
                resolver.reject(error.clone());
            }
        }
    }

    fn attempt_flush<T: AsyncWrite + ?Sized>(
        &mut self,
        stream: Pin<&mut T>,
        writable_context: &mut Context<'_>,
        connection_killswitch: &KillSwitch,
    ) -> Result<()> {
        let res = self.flush(stream, writable_context);
        self.handle_io_result(connection_killswitch, res)
    }

    fn handle_io_result(
        &mut self,
        connection_killswitch: &KillSwitch,
        result: Result<()>,
    ) -> Result<()> {
        if let Err(e) = self.socket_state.handle_io_result(result) {
            error!(error=?e, "error doing IO");
            self.critical_error(connection_killswitch, e)?;
        }
        Ok(())
    }

    fn flush<T: AsyncWrite + ?Sized>(
        &mut self,
        stream: Pin<&mut T>,
        writable_context: &mut Context<'_>,
    ) -> Result<()> {
        let res = stream.poll_flush(writable_context)?;
        self.socket_state.handle_write_poll(res);
        Ok(())
    }

    fn write<T: AsyncWrite + ?Sized>(
        &mut self,
        mut stream: Pin<&mut T>,
        writable_context: &mut Context<'_>,
        connection_killswitch: &KillSwitch,
    ) -> Result<()> {
        while self.can_write() {
            let res =
                self.write_to_stream(stream.as_mut(), writable_context, connection_killswitch);
            self.handle_io_result(connection_killswitch, res)?;
        }
        Ok(())
    }

    fn read<T: AsyncRead + ?Sized>(
        &mut self,
        mut stream: Pin<&mut T>,
        readable_context: &mut Context<'_>,
        connection_killswitch: &KillSwitch,
    ) -> Result<()> {
        while self.can_read() {
            let res =
                self.read_from_stream(stream.as_mut(), readable_context, connection_killswitch);
            let stop = res.as_ref().is_ok_and(|stop| *stop);
            self.handle_io_result(connection_killswitch, res.map(|_| ()))?;
            if stop {
                break;
            }
        }
        Ok(())
    }

    fn write_to_stream<T: AsyncWrite + ?Sized>(
        &mut self,
        mut stream: Pin<&mut T>,
        writable_context: &mut Context<'_>,
        connection_killswitch: &KillSwitch,
    ) -> Result<()> {
        self.flush(stream.as_mut(), writable_context)?;
        self.serialize(connection_killswitch)?;

        let res = self
            .send_buffer
            .poll_write_to(writable_context, stream.as_mut())?;

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
                                resolver.resolve(());
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

                self.flush(stream, writable_context)?;
            } else {
                error!("Socket was writable but we wrote 0, marking as wouldblock");
                self.socket_state.handle_write_poll::<()>(Poll::Pending);
            }
        }
        Ok(())
    }

    fn read_from_stream<T: AsyncRead + ?Sized>(
        &mut self,
        stream: Pin<&mut T>,
        readable_context: &mut Context<'_>,
        connection_killswitch: &KillSwitch,
    ) -> Result<bool> {
        match self.connection_status.state() {
            ConnectionState::Closed => Ok(true),
            ConnectionState::Error => {
                Err(ErrorKind::InvalidConnectionState(ConnectionState::Error).into())
            }
            _ => {
                let res = self
                    .receive_buffer
                    .poll_read_from(readable_context, stream)?;

                if let Some(sz) = self.socket_state.handle_read_poll(res) {
                    if sz > 0 {
                        self.heartbeat.update_last_read();

                        trace!("read {} bytes", sz);
                        self.receive_buffer.fill(sz);
                    } else {
                        error!(
                            "Socket was readable but we read 0. This usually means that the connection is half closed, thus report it as broken."
                        );
                        // Give a chance to parse and use frames we already read from socket before overriding the error with a custom one.
                        if !self.handle_frames(connection_killswitch)?
                            && self.internal_rpc.is_empty()
                        {
                            if self.reconnecting() || self.channels.connection_killswitch().killed()
                            {
                                trace!(
                                    "We're in the process of recovering connection, quit reading socket to enter recovery"
                                );
                                return Ok(true);
                            }
                            self.socket_state.handle_io_result(Err(io::Error::from(
                                io::ErrorKind::ConnectionAborted,
                            )
                            .into()))?;
                        }
                    }
                }
                Ok(false)
            }
        }
    }

    fn serialize(&mut self, connection_killswitch: &KillSwitch) -> Result<()> {
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
                            self.critical_error(
                                connection_killswitch,
                                ErrorKind::SerialisationError(Arc::new(e)).into(),
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_frames(&mut self, connection_killswitch: &KillSwitch) -> Result<bool> {
        let mut did_something = false;
        while self.can_parse() {
            if let Some(frame) = self.parse(connection_killswitch)? {
                self.channels.handle_frame(frame)?;
                did_something = true;
            } else {
                break;
            }
        }
        Ok(did_something)
    }

    fn parse(&mut self, connection_killswitch: &KillSwitch) -> Result<Option<AMQPFrame>> {
        match parse_frame(self.receive_buffer.parsing_context()) {
            Ok((i, f)) => {
                let consumed = self.receive_buffer.offset(i);
                let frame_max = self.configuration.frame_max() as usize;
                if frame_max > 0 && consumed > frame_max {
                    error!(bytes = consumed, "received large frame");
                    let error = AMQPError::new(
                        AMQPHardError::FRAMEERROR.into(),
                        format!("frame too large: {consumed} bytes").into(),
                    );
                    self.internal_rpc.close_connection(
                        error.get_id(),
                        error.get_message().to_string(),
                        0,
                        0,
                    );
                    self.critical_error(
                        connection_killswitch,
                        ErrorKind::ProtocolError(error).into(),
                    )?;
                }
                self.receive_buffer.consume(consumed);
                Ok(Some(f))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    error!(error=?e, "parse error");
                    self.critical_error(connection_killswitch, ErrorKind::ParsingError(e).into())?;
                }
                Ok(None)
            }
        }
    }
}
