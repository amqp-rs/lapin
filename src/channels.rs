use crate::{
    BasicProperties, Channel, ChannelState, Configuration, Connection, ConnectionProperties,
    ConnectionState, ConnectionStatus, Error, ErrorKind, Promise, Result,
    connection_closer::ConnectionCloser,
    error_handler::ErrorHandler,
    frames::Frames,
    heartbeat::Heartbeat,
    id_sequence::IdSequence,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    protocol::{AMQPClass, AMQPError, AMQPHardError},
    recovery_config::RecoveryConfig,
    socket_state::SocketStateHandle,
    types::{ChannelId, Identifier, PayloadSize},
    uri::AMQPUri,
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use executor_trait::FullExecutor;
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};
use tracing::{Level, debug, error, level_enabled, trace};

#[derive(Clone)]
pub(crate) struct Channels {
    inner: Arc<Mutex<Inner>>,
    channel0: Channel,
    configuration: Configuration,
    connection_status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
    executor: Arc<dyn FullExecutor + Send + Sync>,
    frames: Frames,
    heartbeat: Heartbeat,
    connection_killswitch: KillSwitch,
    error_handler: ErrorHandler,
    uri: AMQPUri,
    options: ConnectionProperties,
    recovery_config: RecoveryConfig,
}

impl Channels {
    pub(crate) fn new(
        configuration: Configuration,
        connection_status: ConnectionStatus,
        waker: SocketStateHandle,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        heartbeat: Heartbeat,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        uri: AMQPUri,
        options: ConnectionProperties,
    ) -> Self {
        let recovery_config = options.recovery_config.clone().unwrap_or_default();
        let mut inner = Inner::new(configuration.clone(), waker, recovery_config.clone());
        let channel0 = inner.create_channel(
            0,
            connection_status.clone(),
            internal_rpc.clone(),
            frames.clone(),
            executor.clone(),
            None,
        );
        channel0.set_state(ChannelState::Connected);

        Self {
            inner: Arc::new(Mutex::new(inner)),
            channel0,
            configuration,
            connection_status,
            internal_rpc,
            executor,
            frames,
            heartbeat,
            connection_killswitch: KillSwitch::default(),
            error_handler: ErrorHandler::default(),
            uri,
            options,
            recovery_config,
        }
    }

    pub(crate) fn create(&self, connection_closer: Arc<ConnectionCloser>) -> Result<Channel> {
        self.lock_inner().create(
            self.connection_status.clone(),
            self.internal_rpc.clone(),
            self.frames.clone(),
            self.executor.clone(),
            connection_closer,
        )
    }

    pub(crate) fn channel0(&self) -> Channel {
        self.channel0.clone()
    }

    pub(crate) fn recovery_config(&self) -> RecoveryConfig {
        self.recovery_config.clone()
    }

    pub(crate) fn get(&self, id: ChannelId) -> Option<Channel> {
        if id == 0 {
            Some(self.channel0())
        } else {
            self.lock_inner().channels.get(&id).cloned()
        }
    }

    pub(crate) fn remove(&self, id: ChannelId, error: Error) -> Result<()> {
        self.frames.clear_expected_replies(id, error);

        if id == 0 {
            return Ok(());
        }

        if self.lock_inner().channels.remove(&id).is_some() {
            Ok(())
        } else {
            Err(ErrorKind::InvalidChannel(id).into())
        }
    }

    pub(crate) fn connection_killswitch(&self) -> KillSwitch {
        self.connection_killswitch.clone()
    }

    pub(crate) fn receive_method(&self, id: ChannelId, method: AMQPClass) -> Result<()> {
        if id == 0 {
            self.channel0.receive_method(method)
        } else {
            self.get(id)
                .map(|channel| channel.receive_method(method))
                .unwrap_or_else(|| Err(ErrorKind::InvalidChannel(id).into()))
        }
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        id: ChannelId,
        class_id: Identifier,
        size: PayloadSize,
        properties: BasicProperties,
    ) -> Result<()> {
        self.get(id)
            .map(|channel| channel.handle_content_header_frame(class_id, size, properties))
            .unwrap_or_else(|| Err(ErrorKind::InvalidChannel(id).into()))
    }

    pub(crate) fn handle_body_frame(&self, id: ChannelId, payload: Vec<u8>) -> Result<()> {
        self.get(id)
            .map(|channel| channel.handle_body_frame(payload))
            .unwrap_or_else(|| Err(ErrorKind::InvalidChannel(id).into()))
    }

    pub(crate) fn set_connection_closing(&self) {
        self.heartbeat.cancel();
        self.connection_status.set_state(ConnectionState::Closing);
        for channel in self.lock_inner().channels.values() {
            channel.set_closing(None);
        }
    }

    pub(crate) fn set_connection_closed(&self, error: Error) {
        self.connection_status.set_state(ConnectionState::Closed);
        for (id, channel) in self.lock_inner().channels.iter() {
            self.frames.clear_expected_replies(*id, error.clone());
            channel.set_closed(error.clone());
        }
    }

    pub(crate) fn set_connection_error(&self, error: Error) {
        if self.recovery_config.can_recover_connection(&error) {
            if self.connection_status.connected() {
                self.connection_killswitch.kill();
                self.init_connection_recovery(error);
            }
            return;
        }

        // Do nothing if we were already in error
        if let ConnectionState::Error = self.connection_status.set_state(ConnectionState::Error) {
            return;
        }

        error!(%error, "Connection error");
        if let Some(resolver) = self.connection_status.connection_resolver() {
            resolver.reject(error.clone());
        }

        self.frames.drop_pending(error.clone());
        self.error_handler.on_error(error.clone());
        for (id, channel) in self.lock_inner().channels.iter() {
            self.frames.clear_expected_replies(*id, error.clone());
            channel.set_connection_error(error.clone());
        }
    }

    pub(crate) fn flow(&self) -> bool {
        self.lock_inner()
            .channels
            .values()
            .all(|c| c.status().flow())
    }

    pub(crate) fn send_heartbeat(&self) {
        debug!("send heartbeat");

        let (promise, resolver) = Promise::new();

        if level_enabled!(Level::TRACE) {
            promise.set_marker("Heartbeat".into());
        }

        self.channel0
            .send_frame(AMQPFrame::Heartbeat(0), resolver, None);
        self.internal_rpc.register_internal_future(promise);
    }

    pub(crate) fn handle_frame(&self, f: AMQPFrame) -> Result<()> {
        if let Err(err) = self.do_handle_frame(f) {
            self.set_connection_error(err.clone());
            Err(err)
        } else {
            Ok(())
        }
    }

    fn do_handle_frame(&self, f: AMQPFrame) -> Result<()> {
        trace!(frame=?f, "will handle frame");
        match f {
            AMQPFrame::ProtocolHeader(version) => {
                error!(
                    "we asked for AMQP {} but the server only supports AMQP {}",
                    ProtocolVersion::amqp_0_9_1(),
                    version
                );
                let error: Error = ErrorKind::InvalidProtocolVersion(version).into();
                if let Some(resolver) = self.connection_status.connection_resolver() {
                    resolver.reject(error.clone());
                }
                return Err(error);
            }
            AMQPFrame::Method(channel_id, method) => {
                self.receive_method(channel_id, method)?;
            }
            AMQPFrame::Heartbeat(channel_id) => {
                if channel_id == 0 {
                    debug!("received heartbeat from server");
                } else {
                    error!(channel=%channel_id, "received invalid heartbeat");
                    let error = AMQPError::new(
                        AMQPHardError::FRAMEERROR.into(),
                        format!("heartbeat frame received on channel {channel_id}").into(),
                    );
                    let channel0 = self.channel0();
                    {
                        let error = error.clone();
                        self.internal_rpc.register_internal_future(async move {
                            channel0
                                .connection_close(
                                    error.get_id(),
                                    error.get_message().as_str(),
                                    0,
                                    0,
                                )
                                .await
                        });
                    }
                    return Err(ErrorKind::ProtocolError(error).into());
                }
            }
            AMQPFrame::Header(channel_id, class_id, header) => {
                if channel_id == 0 {
                    error!(channel=%channel_id, "received content header");
                    let error = AMQPError::new(
                        AMQPHardError::CHANNELERROR.into(),
                        format!("content header frame received on channel {channel_id}").into(),
                    );
                    let channel0 = self.channel0();
                    {
                        let error = error.clone();
                        self.internal_rpc.register_internal_future(async move {
                            channel0
                                .connection_close(
                                    error.get_id(),
                                    error.get_message().as_str(),
                                    class_id,
                                    0,
                                )
                                .await
                        });
                    }
                    return Err(ErrorKind::ProtocolError(error).into());
                } else {
                    self.handle_content_header_frame(
                        channel_id,
                        class_id,
                        header.body_size,
                        header.properties,
                    )?;
                }
            }
            AMQPFrame::Body(channel_id, payload) => {
                self.handle_body_frame(channel_id, payload)?;
            }
        }
        Ok(())
    }

    pub(crate) fn set_error_handler<E: FnMut(Error) + Send + 'static>(&self, handler: E) {
        self.error_handler.set_handler(handler);
    }

    pub(crate) fn start_heartbeat(&self) {
        let heartbeat = self.configuration.heartbeat();
        if heartbeat != 0 {
            let heartbeat = Duration::from_millis(u64::from(heartbeat) * 500); // * 1000 (ms) / 2 (half the negotiated timeout)
            self.heartbeat.set_timeout(heartbeat);
            self.heartbeat.start(self.clone());
        }
    }

    pub(crate) fn init_connection_recovery(&self, error: Error) -> Error {
        trace!("init connection recovery");
        self.heartbeat.reset();
        self.connection_status.set_reconnecting();
        let error = self
            .lock_inner()
            .channels
            .values()
            .fold(error, |error, channel| channel.init_recovery(error));
        self.channel0.init_recovery(error)
    }

    pub(crate) async fn start_recovery(&self) -> Result<()> {
        let channels = self
            .lock_inner()
            .channels
            .values()
            .cloned()
            .collect::<Vec<_>>();

        self.connection_killswitch.reset();
        self.frames.drop_poison();
        self.channel0.update_recovery();
        self.channel0.finalize_connection();

        Connection::for_reconnect(
            self.configuration.clone(),
            self.connection_status.clone(),
            self.clone(),
            self.internal_rpc.clone(),
        )
        .start(self.uri.clone(), self.options.clone())
        .await?;

        trace!("Connection recovered, now recovering channels");

        // TODO: use future::join! when stable
        for channel in channels {
            channel.start_recovery().await?;
        }

        Ok(())
    }

    pub(crate) fn init_connection_shutdown(&self, error: Error) {
        let connection_resolver = self.connection_status.connection_resolver();
        self.set_connection_closing();
        self.frames.drop_pending(error.clone());
        if let Some(resolver) = connection_resolver {
            resolver.reject(error.clone());
        }
    }

    pub(crate) fn finish_connection_shutdown(&self) {
        self.connection_killswitch.kill();
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for Channels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Channels");
        if let Ok(inner) = self.inner.try_lock() {
            debug
                .field("channels", &inner.channels.values())
                .field("channel_id", &inner.channel_id)
                .field("configuration", &inner.configuration);
        }
        debug
            .field("frames", &self.frames)
            .field("connection_status", &self.connection_status)
            .field("error_handler", &self.error_handler)
            .finish()
    }
}

struct Inner {
    channels: HashMap<ChannelId, Channel>,
    channel_id: IdSequence<ChannelId>,
    configuration: Configuration,
    waker: SocketStateHandle,
    recovery_config: RecoveryConfig,
}

impl Inner {
    fn new(
        configuration: Configuration,
        waker: SocketStateHandle,
        recovery_config: RecoveryConfig,
    ) -> Self {
        Self {
            channels: HashMap::default(),
            channel_id: IdSequence::new(false),
            configuration,
            waker,
            recovery_config,
        }
    }

    fn create_channel(
        &mut self,
        id: ChannelId,
        connection_status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        connection_closer: Option<Arc<ConnectionCloser>>,
    ) -> Channel {
        debug!(%id, "create channel");
        Channel::new(
            id,
            self.configuration.clone(),
            connection_status,
            self.waker.clone(),
            internal_rpc,
            frames,
            executor,
            connection_closer,
            self.recovery_config.clone(),
        )
    }

    fn create(
        &mut self,
        connection_status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        connection_closer: Arc<ConnectionCloser>,
    ) -> Result<Channel> {
        debug!("create channel");
        self.channel_id.set_max(self.configuration.channel_max());
        let first_id = self.channel_id.next();
        let mut id = first_id;
        let mut met_first_id = false;
        loop {
            if id == first_id {
                if met_first_id {
                    break;
                }
                met_first_id = true;
            }
            if !self.channels.contains_key(&id) {
                let channel = self.create_channel(
                    id,
                    connection_status,
                    internal_rpc,
                    frames,
                    executor,
                    Some(connection_closer),
                );
                self.channels.insert(id, channel.clone_internal());
                return Ok(channel);
            }
            id = self.channel_id.next();
        }
        Err(ErrorKind::ChannelsLimitReached.into())
    }
}
