use crate::{
    AsyncTcpStream, ConnectionState, ConnectionStatus, Error, ErrorKind, Result,
    buffer::{Buffer, FRAME_OVERHEAD},
    channels::Channels,
    configuration::NegotiatedConfig,
    frames::{FrameSending, Frames},
    heartbeat::Heartbeat,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    protocol::{AMQPError, AMQPHardError},
    runtime,
    socket_state::SocketState,
    thread::JoinHandle,
    types::FrameSize,
    uri::AMQPUri,
};
use amq_protocol::frame::{AMQPFrame, GenError, gen_frame, parse_frame};
use async_rs::{Runtime, traits::*};
use backon::{BackoffBuilder, ExponentialBackoff, ExponentialBuilder, Retryable};
use futures_io::{AsyncRead, AsyncWrite};
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    thread::Builder as ThreadBuilder,
    time::Duration,
};
use tracing::{Level, debug, error, trace};

const INITIAL_BUFFER_SIZE: usize = 64 * 1024;
/// How often connecting and throttling check whether the connection is shutting down.
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug, PartialEq)]
enum Status {
    Initial,
    Connected,
    Stop,
}

pub(crate) struct IoLoop<
    RK: RuntimeKit + Clone + Send + 'static,
    C: AsyncFn(AMQPUri, Runtime<RK>) -> Result<AsyncTcpStream<<RK as Reactor>::TcpStream>>
        + Send
        + Sync
        + 'static,
> {
    connection_status: ConnectionStatus,
    configuration: NegotiatedConfig,
    channels: Channels,
    internal_rpc: InternalRPCHandle,
    frames: Frames,
    socket_state: SocketState,
    heartbeat: Heartbeat<RK>,
    runtime: Runtime<RK>,
    connect: C,
    uri: AMQPUri,
    backoff: ExponentialBuilder,
    global_backoff: ExponentialBackoff,
    status: Status,
    receive_buffer: Buffer,
    send_buffer: Buffer,
    serialized_frames: VecDeque<(FrameSize, FrameSending)>,
    half_closed: bool,
    first_connection: bool,
    handshake_done: bool,
}

impl<
    RK: RuntimeKit + Clone + Send + 'static,
    C: AsyncFn(AMQPUri, Runtime<RK>) -> Result<AsyncTcpStream<<RK as Reactor>::TcpStream>>
        + Send
        + Sync
        + 'static,
> IoLoop<RK, C>
{
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        connection_status: ConnectionStatus,
        configuration: NegotiatedConfig,
        channels: Channels,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        socket_state: SocketState,
        heartbeat: Heartbeat<RK>,
        runtime: Runtime<RK>,
        connect: C,
        uri: AMQPUri,
        backoff: ExponentialBuilder,
    ) -> Self {
        let global_backoff = backoff.build();

        Self {
            connection_status,
            configuration,
            channels,
            internal_rpc,
            frames,
            socket_state,
            heartbeat,
            runtime,
            connect,
            uri,
            backoff,
            global_backoff,
            status: Status::Initial,
            receive_buffer: Buffer::with_capacity(INITIAL_BUFFER_SIZE),
            send_buffer: Buffer::with_capacity(INITIAL_BUFFER_SIZE),
            serialized_frames: VecDeque::default(),
            half_closed: false,
            first_connection: true,
            handshake_done: false,
        }
    }

    fn reset(&mut self) {
        self.status = Status::Initial;
        self.handshake_done = false;
        self.receive_buffer.reset();
        self.send_buffer.reset();
        self.socket_state.reset();
    }

    fn finish_setup(&mut self) -> Result<bool> {
        if self.connection_status.connected() {
            let heartbeat = self.configuration.heartbeat();
            if heartbeat != 0 {
                let heartbeat = Duration::from_millis(u64::from(heartbeat) * 500); // * 1000 (ms) / 2 (half the negotiated timeout)
                self.internal_rpc.start_heartbeat(heartbeat);
            }
            self.internal_rpc.start_secret_update();
            self.status = Status::Connected;
            self.handshake_done = true;
            self.global_backoff = self.backoff.build();
            self.first_connection = false;
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
        self.socket_state.writable()
            && !self.connection_status.blocked()
            && (self.send_buffer.available_data() > 0 || self.frames.has_ready())
    }

    fn can_read(&mut self) -> bool {
        self.socket_state.readable() && self.receive_buffer.available_space() > 0
    }

    fn can_parse(&self) -> bool {
        self.receive_buffer.available_data() > 0
    }

    fn frame_limit(&self) -> usize {
        // The negotiated limit applies once this transport completed its
        // handshake, even once it stops.
        if !self.handshake_done {
            self.configuration.pre_tune_frame_max() as usize
        } else {
            self.configuration.frame_max() as usize
        }
    }

    fn connecting(&self) -> bool {
        self.status == Status::Initial
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
            Status::Initial => !self.half_closed,
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

    pub(crate) fn start(mut self) -> Result<JoinHandle> {
        let runtime = self.runtime.clone();
        let waker = self.socket_state.handle();
        let connect_span = tracing::Span::current();
        let handle = ThreadBuilder::new()
            .name("lapin-io-loop".to_owned())
            .spawn(move || {
                let loop_span = io_loop_span(connect_span);
                let _enter = loop_span.enter();
                let connection_killswitch = self.channels.connection_killswitch();
                let readable_waker = self.socket_state.readable_waker();
                let mut readable_context = Context::from_waker(&readable_waker);
                let writable_waker = self.socket_state.writable_waker();
                let mut writable_context = Context::from_waker(&writable_waker);
                let (mut stream, res) = loop {
                    let connect_result = {
                    let connect = || (self.connect)(self.uri.clone(), self.runtime.clone());
                    let runtime = self.runtime.clone();
                    let connect = connect.retry(self.backoff).sleep(move |dur| runtime.sleep(dur));
                    let status = self.connection_status.clone();
                    let cancel_runtime = self.runtime.clone();
                    let cancellation = async move {
                        loop {
                            let state = status.state();
                            if matches!(
                                state,
                                ConnectionState::Closing
                                    | ConnectionState::Closed
                                    | ConnectionState::Error
                            ) {
                                return Err(ErrorKind::InvalidConnectionState(state).into());
                            }
                            cancel_runtime.sleep(SHUTDOWN_POLL_INTERVAL).await;
                        }
                    };
                    self.runtime.block_on(runtime::race(cancellation, connect))
                    };
                    let mut stream = match connect_result {
                        Ok(stream) => stream,
                        Err(err) => {
                            trace!("Poison connection attempt");
                            self.connection_status.poison(err.clone());
                            self.frames.clear_connection_steps(&err);
                            // A dial canceled by a close ends like a close
                            // during the throttle.
                            let result = if matches!(
                                self.connection_status.state(),
                                ConnectionState::Closing | ConnectionState::Closed
                            ) {
                                Ok(())
                            } else {
                                Err(err)
                            };
                            self.internal_rpc.stop();
                            return result;
                        }
                    };
                    self.half_closed = false;
                    let mut res = Ok(());

                    while self.should_continue(&connection_killswitch) {
                        if let Err(err) = self.run(Pin::new(&mut stream), &mut readable_context, &mut writable_context, &connection_killswitch) {
                            let io_err = err.is_io_error();
                            res = self.critical_error(&connection_killswitch, err);
                            if self.connecting() && io_err {
                                break;
                            }
                        }
                    }

                    let connecting = self.connecting();
                    let reconnect = self.reconnecting();

                    trace!(status=?self.status, connection_status=?self.connection_status.state(), "io_loop exiting for {}", if reconnect { "reconnection" } else if connecting { "connection" } else { "shutdown" });
                    if self.first_connection && connecting && self.half_closed && res.is_ok() {
                        let error = Error::from(io::Error::from(io::ErrorKind::ConnectionAborted));
                        if self.frames.retry_connection_header() {
                            // Before Connection.Start, retry the protocol header with
                            // the same promise, including if it was only partly sent.
                            res = Err(error);
                        } else {
                            // Later handshake stages may own asynchronous auth work;
                            // do not carry that exchange onto a fresh TCP connection.
                            self.channels.set_connection_error(error.clone());
                            self.clear_serialized_frames(error.clone());
                            break (stream, Err(error));
                        }
                    }
                    self.clear_serialized_frames(self.frames.poison().or_else(|| res.clone().err()).unwrap_or(
                        ErrorKind::InvalidConnectionState(ConnectionState::Closed).into(),
                    ));

                    if !connecting && !reconnect {
                        break (stream, res);
                    }
                    if self.connection_status.closed() {
                        // Closed during this attempt: whatever is left of the
                        // backoff, the close was requested.
                        break (stream, self.end_closed());
                    }

                    let throttle = match self.global_backoff.next() {
                        Some(throttle) => throttle,
                        None => {
                            error!("Exponential backoff attempts exhausted, aborting recovery");
                            let error = res.err().unwrap_or_else(|| {
                                io::Error::from(io::ErrorKind::ConnectionAborted).into()
                            });
                            self.channels.set_connection_error(error.clone());
                            break (stream, Err(error));
                        }
                    };
                    debug!("Throttling {:?} before reconnection to avoid flooding", throttle);
                    let started = std::time::Instant::now();
                    while let Some(remaining) = throttle.checked_sub(started.elapsed()) {
                        if self.connection_status.closing()
                            || self.connection_status.closed()
                            || self.connection_status.errored()
                        {
                            break;
                        }
                        std::thread::sleep(remaining.min(SHUTDOWN_POLL_INTERVAL));
                    }

                    if let Some(result) = self.end_after_throttle(&res) {
                        break (stream, result);
                    }

                    self.reset();
                    if !self.first_connection {
                        self.internal_rpc.start_channels_recovery();
                    }
                };

                trace!(status=?self.status, connection_status=?self.connection_status.state(), "io_loop entering exit/cleanup phase");
                self.internal_rpc.stop();
                if self.heartbeat.killswitch().killed()
                    && let Err(err) = self.runtime.block_on(std::future::poll_fn(move |cx| {
                        Pin::new(&mut stream)
                            .poll_close(cx)
                    })) {
                        error!(?err, "Failed to close IO stream");
                    }
                res
            }).map_err(|err| Error::io(err, &runtime))?;
        waker.wake();
        Ok(handle)
    }

    fn stop(&mut self) {
        self.status = Status::Stop;
    }

    /// How the IO loop ends when the connection was closed or failed during
    /// the reconnect throttle, if it was.
    fn end_after_throttle(&mut self, res: &Result<()>) -> Option<Result<()>> {
        let state = self.connection_status.state();
        // A protocol violation starts a close before it fails the connection:
        // until the dispatcher fails it, the attempt's error tells it apart
        // from a requested close.
        let failed = res.as_ref().is_err_and(|err| !err.is_io_error());
        match state {
            ConnectionState::Closed => Some(self.end_closed()),
            ConnectionState::Closing if !failed => Some(self.end_closed()),
            // A failed connection ends in Error, whichever of this thread and
            // the dispatcher gets there first.
            ConnectionState::Closing | ConnectionState::Error => {
                let error = res
                    .clone()
                    .err()
                    .unwrap_or_else(|| ErrorKind::InvalidConnectionState(state).into());
                self.channels.set_connection_error(error.clone());
                Some(Err(error))
            }
            _ => None,
        }
    }

    /// End a connection whose close was requested while it was recovering.
    fn end_closed(&mut self) -> Result<()> {
        let error: Error = ErrorKind::InvalidConnectionState(ConnectionState::Closed).into();
        self.frames.drop_pending(error.clone(), &self.internal_rpc);
        self.channels.set_connection_closed(error);
        Ok(())
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
        if !self.connection_status.closing() && self.channels.can_recover(&error) {
            self.internal_rpc.init_connection_recovery(error.clone());
            return Err(error);
        }

        if let Some(resolver) = self.frames.connection_resolver(0) {
            resolver.reject(error.clone());
        }
        self.stop();
        self.internal_rpc.set_connection_error(error.clone());
        self.clear_serialized_frames(error.clone());
        Err(error)
    }

    fn clear_serialized_frames(&mut self, error: Error) {
        if !self.serialized_frames.is_empty() {
            trace!(
                "We're quitting but had leftover frames, tag them as 'not sent' with current error"
            );
            for (_, sending) in std::mem::take(&mut self.serialized_frames) {
                sending.reject(error.clone());
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
            if e.is_runtime_shutdown_error() {
                trace!(error=?e, "runtime shutdown error doing IO");
            } else {
                error!(error=?e, "error doing IO");
            }
            self.critical_error(connection_killswitch, e)?;
        }
        Ok(())
    }

    fn flush<T: AsyncWrite + ?Sized>(
        &mut self,
        stream: Pin<&mut T>,
        writable_context: &mut Context<'_>,
    ) -> Result<()> {
        let res = stream
            .poll_flush(writable_context)
            .map_err(|err| Error::io(err, &self.runtime))?;
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
            let res = self.write_to_stream(stream.as_mut(), writable_context);
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
    ) -> Result<()> {
        self.flush(stream.as_mut(), writable_context)?;
        self.serialize()?;

        if self.send_buffer.available_data() == 0 {
            return Ok(());
        }

        let res = self
            .send_buffer
            .poll_write_to(writable_context, stream.as_mut())
            .map_err(|err| Error::io(err, &self.runtime))?;

        if let Some(sz) = self.socket_state.handle_write_poll(res) {
            if sz > 0 {
                self.heartbeat.update_last_write();

                trace!("wrote {} bytes", sz);
                self.send_buffer.consume(sz);

                let mut written = sz as FrameSize;
                while written > 0 {
                    if let Some((to_write, sending)) = self.serialized_frames.pop_front() {
                        if written < to_write {
                            self.serialized_frames
                                .push_front((to_write - written, sending));
                            trace!("{} to write to complete this frame", to_write - written);
                            written = 0;
                        } else {
                            sending.resolve();
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
                    .poll_read_from(readable_context, stream)
                    .map_err(|err| Error::io(err, &self.runtime))?;

                if let Some(sz) = self.socket_state.handle_read_poll(res) {
                    if sz > 0 {
                        self.heartbeat.update_last_read();

                        trace!("read {} bytes", sz);
                        self.receive_buffer.fill(sz);
                    } else if self.half_closed {
                        return self.handle_half_closed_connection(false);
                    } else {
                        error!(
                            "Socket was readable but we read 0. This usually means that the connection is half closed, thus report it as broken."
                        );
                        self.half_closed = true;
                        // Give a chance to parse and use frames we already read from socket before overriding the error with a custom one.
                        if !self.handle_frames(connection_killswitch)?
                            && self.internal_rpc.is_empty()
                        {
                            return self.handle_half_closed_connection(true);
                        }
                    }
                }
                Ok(false)
            }
        }
    }

    fn handle_half_closed_connection(&mut self, first_event: bool) -> Result<bool> {
        if self.reconnecting()
            || self.connecting()
            || self.channels.connection_killswitch().killed()
        {
            if first_event {
                trace!(
                    "We're in the process of recovering connection, quit reading socket to enter recovery"
                );
            }
            return Ok(true);
        }
        self.socket_state
            .handle_io_result(Err(io::Error::from(io::ErrorKind::ConnectionAborted).into()))?;
        Ok(false)
    }

    fn serialize(&mut self) -> Result<()> {
        while let Some(next_msg) = self.frames.pop() {
            // A body frame that cannot fit behind the data already buffered
            // waits for that data to be written, without being copied first.
            if let AMQPFrame::Body(_, payload) = &*next_msg
                && self.send_buffer.available_data() > 0
                && self.send_buffer.available_space() < payload.len() + FRAME_OVERHEAD
            {
                self.frames.retry(next_msg);
                break;
            }
            trace!("will write frame to buffer");
            let checkpoint = self.send_buffer.checkpoint();
            let res = gen_frame(&next_msg)((&mut self.send_buffer).into());
            match res.map(|w| w.into_inner().1) {
                Ok(sz) if sz > self.frame_limit() as u64 => {
                    self.send_buffer.rollback(checkpoint);
                    let error: Error =
                        ErrorKind::SerialisationError(Arc::new(GenError::BufferTooSmall(
                            usize::try_from(sz - self.frame_limit() as u64).unwrap_or(usize::MAX),
                        )))
                        .into();
                    next_msg.reject(error.clone());
                    return Err(error);
                }
                Ok(sz) => self
                    .serialized_frames
                    .push_back(next_msg.into_serialized_frame(sz as FrameSize)),
                Err(e) => {
                    self.send_buffer.rollback(checkpoint);
                    match e {
                        GenError::BufferTooSmall(needed) => {
                            self.frames.retry(next_msg);
                            if self.send_buffer.available_data() > 0 {
                                break;
                            }
                            let capacity = self.send_buffer.capacity();
                            let target = capacity
                                .saturating_add(needed)
                                .max(capacity.saturating_mul(2))
                                .min(self.frame_limit());
                            if target <= capacity || self.send_buffer.grow(target).is_err() {
                                return Err(ErrorKind::SerialisationError(Arc::new(
                                    GenError::BufferTooSmall(needed),
                                ))
                                .into());
                            }
                        }
                        e => {
                            error!(error=?e, "error generating frame");
                            let error: Error = ErrorKind::SerialisationError(Arc::new(e)).into();
                            next_msg.reject(error.clone());
                            return Err(error);
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
                self.channels
                    .handle_frame(frame)
                    .inspect_err(|err| self.internal_rpc.set_connection_error(err.clone()))?;
                did_something = true;
            } else {
                break;
            }
        }
        Ok(did_something)
    }

    fn parse(&mut self, connection_killswitch: &KillSwitch) -> Result<Option<AMQPFrame>> {
        if let Some(wire_len) = self.receive_buffer.frame_wire_len() {
            if wire_len > self.frame_limit() as u64 {
                let error = AMQPError::new(
                    AMQPHardError::FRAMEERROR.into(),
                    format!("frame too large: {wire_len} bytes").into(),
                );
                self.channels
                    .channel0()
                    .report_protocol_violation(error, 0, 0)?;
            }
            let wire_len = wire_len as usize;
            if wire_len > self.receive_buffer.capacity()
                && let Err(err) = self.receive_buffer.grow(wire_len)
            {
                // The frame is within the negotiated size: this transport
                // is fine, but the connection cannot go on without it.
                error!(bytes = wire_len, %err, "cannot allocate the receive buffer for a frame");
                let error = AMQPError::new(
                    AMQPHardError::RESOURCEERROR.into(),
                    format!("cannot buffer a frame of {wire_len} bytes").into(),
                );
                self.channels
                    .channel0()
                    .report_protocol_violation(error, 0, 0)?;
            }
        }
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
                    self.channels
                        .channel0()
                        .report_protocol_violation(error, 0, 0)
                        .or_else(|err| self.critical_error(connection_killswitch, err))?;
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

/// Create a new span for the io_loop thread that follows from the connect span, and has the same level.
///
/// Importantly, we drop the `connect_span` so it closes properly.
fn io_loop_span(connect_span: tracing::Span) -> tracing::Span {
    let span_level = connect_span.metadata().map_or(Level::INFO, |m| *m.level());
    // tracing::span! requires a const level, so we must match exhaustively instead of
    // forwarding the level dynamically. The wildcard is intentionally absent so a future
    // tracing Level addition produces a compile error rather than a silent wrong level.
    let span = match span_level {
        Level::TRACE => tracing::span!(Level::TRACE, "io_loop"),
        Level::DEBUG => tracing::span!(Level::DEBUG, "io_loop"),
        Level::INFO => tracing::span!(Level::INFO, "io_loop"),
        Level::WARN => tracing::span!(Level::WARN, "io_loop"),
        Level::ERROR => tracing::span!(Level::ERROR, "io_loop"),
    };

    // This span doesn't contribute to the duration of the connect span, but it is caused by the
    // connect operation, so we set it as a follows_from relationship.
    span.follows_from(&connect_span);
    span
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConnectionProperties, configuration::Configuration, events::Events,
        internal_rpc::InternalRPC, runtime, secret_update::SecretUpdate,
    };

    type TestKit = runtime::DefaultRuntimeKit;

    /// An IO loop whose connection is in `state`, with a dispatcher that is
    /// created but not started.
    #[allow(clippy::type_complexity)]
    fn test_io_loop(
        uri: AMQPUri,
        properties: ConnectionProperties,
        state: ConnectionState,
    ) -> (
        IoLoop<
            TestKit,
            impl AsyncFn(
                AMQPUri,
                Runtime<TestKit>,
            ) -> Result<AsyncTcpStream<<TestKit as Reactor>::TcpStream>>
            + Send
            + Sync
            + 'static,
        >,
        InternalRPC<TestKit>,
        Channels,
    ) {
        let runtime = runtime::default_runtime().unwrap();
        let configuration = Configuration::new(&uri, properties);
        let status = ConnectionStatus::new(&uri);
        status.set_state(state);
        let frames = Frames::default();
        let socket_state = SocketState::default();
        let handle = socket_state.handle();
        let heartbeat = Heartbeat::new(status.clone(), runtime.clone());
        let secret_update = SecretUpdate::new(
            status.clone(),
            runtime.clone(),
            configuration.auth_provider.clone(),
        );
        let rpc = InternalRPC::new(
            runtime.clone(),
            heartbeat.clone(),
            secret_update,
            frames.clone(),
            handle.clone(),
        );
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            handle,
            rpc.handle(),
            frames.clone(),
            Events::new(status.clone()),
        );
        let io_loop = IoLoop::new(
            status,
            configuration.negotiated_config,
            channels.clone(),
            rpc.handle(),
            frames,
            socket_state,
            heartbeat,
            runtime,
            async |_, _| Err(Error::other("unexpected connection attempt")),
            uri,
            ExponentialBuilder::default(),
        );
        (io_loop, rpc, channels)
    }

    #[test]
    fn a_transport_stopped_before_tune_keeps_the_pre_tune_frame_limit() {
        let (mut io_loop, _rpc, _channels) = test_io_loop(
            AMQPUri::default(),
            ConnectionProperties::default(),
            ConnectionState::Connecting,
        );
        // The header of a Connection.Start whose payload has not arrived yet.
        io::Write::write_all(&mut &mut io_loop.receive_buffer, &[1, 0, 0, 0, 0, 1, 0]).unwrap();
        io_loop.stop();
        assert!(!io_loop.handle_frames(&KillSwitch::default()).unwrap());
        assert_eq!(io_loop.receive_buffer.available_data(), 7);
    }

    #[test]
    fn after_the_throttle_a_failure_ends_in_error_and_a_close_closed() {
        let (mut io_loop, _rpc, channels) = test_io_loop(
            AMQPUri::default(),
            ConnectionProperties::default().enable_auto_recover(),
            ConnectionState::Connecting,
        );
        io_loop.first_connection = false;
        let violation: Error = ErrorKind::ProtocolError(AMQPError::new(
            AMQPHardError::FRAMEERROR.into(),
            "frame too large".into(),
        ))
        .into();
        // The dispatcher started the violation's close but did not fail the
        // connection yet when the throttle ended.
        channels.set_connection_closing();
        let result = io_loop.end_after_throttle(&Err(violation));
        assert!(matches!(
            result,
            Some(Err(error)) if matches!(error.kind(), ErrorKind::ProtocolError(_))
        ));
        assert!(io_loop.connection_status.errored());

        let (mut io_loop, _rpc, _channels) = test_io_loop(
            AMQPUri::default(),
            ConnectionProperties::default().enable_auto_recover(),
            ConnectionState::Closed,
        );
        assert!(matches!(io_loop.end_after_throttle(&Ok(())), Some(Ok(()))));
        assert!(io_loop.connection_status.closed());

        // A close requested during an attempt that its transport ended.
        let (mut io_loop, _rpc, channels) = test_io_loop(
            AMQPUri::default(),
            ConnectionProperties::default().enable_auto_recover(),
            ConnectionState::Connecting,
        );
        channels.set_connection_closing();
        let retry: Error = io::Error::from(io::ErrorKind::ConnectionAborted).into();
        assert!(matches!(
            io_loop.end_after_throttle(&Err(retry)),
            Some(Ok(()))
        ));
        assert!(io_loop.connection_status.closed());
    }
}
