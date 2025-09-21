use crate::{
    BasicProperties, Channel, ChannelState, Configuration, Connection, ConnectionProperties,
    ConnectionState, ConnectionStatus, Error, ErrorKind, PromiseResolver, Result,
    connection_closer::ConnectionCloser,
    events::{Events, EventsSender},
    frames::Frames,
    id_sequence::IdSequence,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    protocol::{AMQPClass, AMQPError, AMQPHardError},
    recovery_config::RecoveryConfig,
    socket_state::SocketStateHandle,
    types::{ChannelId, Identifier, PayloadSize},
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use tracing::{debug, error, trace};

#[derive(Clone)]
pub(crate) struct Channels {
    inner: Arc<RwLock<Inner>>,
    channel0: Channel,
    configuration: Configuration,
    connection_status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
    frames: Frames,
    connection_killswitch: KillSwitch,
    events: Events,
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
        options: ConnectionProperties,
        events: Events,
    ) -> Self {
        let recovery_config = configuration.recovery_config.clone();
        let mut inner = Inner::new(configuration.clone(), waker, recovery_config.clone());
        let channel0 = inner.create_channel(
            0,
            connection_status.clone(),
            internal_rpc.clone(),
            frames.clone(),
            events.sender(),
            None,
        );
        channel0.set_state(ChannelState::Connected);

        Self {
            inner: Arc::new(RwLock::new(inner)),
            channel0,
            configuration,
            connection_status,
            internal_rpc,
            frames,
            connection_killswitch: KillSwitch::default(),
            events,
            options,
            recovery_config,
        }
    }

    pub(crate) fn create(&self, connection_closer: Arc<ConnectionCloser>) -> Result<Channel> {
        self.write().create(
            self.connection_status.clone(),
            self.internal_rpc.clone(),
            self.frames.clone(),
            self.events.sender(),
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
            self.read().channels.get(&id).cloned()
        }
    }

    pub(crate) fn remove(&self, id: ChannelId, error: Error) -> Result<()> {
        self.frames.clear_expected_replies(id, error);

        if id == 0 {
            return Ok(());
        }

        if self.write().channels.remove(&id).is_some() {
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
        self.connection_status.set_state(ConnectionState::Closing);
        for channel in self.read().channels.values() {
            channel.set_closing(None);
        }
    }

    pub(crate) fn set_connection_closed(&self, error: Error) {
        self.connection_status.set_state(ConnectionState::Closed);
        for channel in self.read().channels.values() {
            channel.set_closed(error.clone());
        }
    }

    pub(crate) fn should_init_connection_recovery(&self) -> bool {
        if self.connection_status.connected() {
            self.connection_killswitch.kill();
            return true;
        }
        false
    }

    pub(crate) fn set_connection_error(&self, error: Error) {
        // Do nothing if we were already in error
        if let ConnectionState::Error = self.connection_status.set_state(ConnectionState::Error) {
            return;
        }

        error!(%error, "Connection error");
        if let Some(resolver) = self.connection_status.connection_resolver() {
            resolver.reject(error.clone());
        }

        self.frames.drop_pending(error.clone(), &self.internal_rpc);
        self.events.sender().error(error.clone());
        for channel in self.read().channels.values() {
            channel.set_connection_error(error.clone());
        }
    }

    pub(crate) fn flow(&self) -> bool {
        self.read().channels.values().all(|c| c.status().flow())
    }

    pub(crate) fn handle_frame(&self, f: AMQPFrame) -> Result<()> {
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
            AMQPFrame::Heartbeat => {
                debug!("received heartbeat from server");
            }
            AMQPFrame::InvalidHeartbeat(channel_id) => {
                error!(channel=%channel_id, "received invalid heartbeat");
                let error = AMQPError::new(
                    AMQPHardError::FRAMEERROR.into(),
                    format!("heartbeat frame received on channel {channel_id}").into(),
                );
                return self.channel0.report_protocol_violation(error, 0, 0);
            }
            AMQPFrame::Header(channel_id, header) => {
                if channel_id == 0 {
                    error!(channel=%channel_id, "received content header");
                    let error = AMQPError::new(
                        AMQPHardError::CHANNELERROR.into(),
                        format!("content header frame received on channel {channel_id}").into(),
                    );
                    return self
                        .channel0
                        .report_protocol_violation(error, header.class_id, 0);
                } else {
                    self.handle_content_header_frame(
                        channel_id,
                        header.class_id,
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

    pub(crate) fn init_connection_recovery(&self, error: Error) -> Error {
        trace!("init connection recovery");
        self.connection_status.set_reconnecting();
        let error = self
            .read()
            .channels
            .values()
            .fold(error, |error, channel| channel.init_recovery(error));
        self.channel0.init_recovery(error)
    }

    pub(crate) async fn start_recovery(&self) -> Result<()> {
        let channels = self.read().channels.values().cloned().collect::<Vec<_>>();

        self.connection_killswitch.reset();
        self.frames.drop_poison();
        self.channel0.update_recovery();
        self.channel0.finalize_connection();

        Connection::for_reconnect(
            self.configuration.clone(),
            self.connection_status.clone(),
            self.internal_rpc.clone(),
            self.events.clone(),
        )
        .start(self.channel0(), self.options.clone())
        .await?;

        trace!("Connection recovered, now recovering channels");

        // TODO: use future::join! when stable
        for channel in channels {
            channel.start_recovery().await?;
        }

        Ok(())
    }

    pub(crate) fn init_connection_shutdown(
        &self,
        error: Error,
        connection_resolver: Option<PromiseResolver<Connection>>,
    ) {
        self.frames.drop_pending(error.clone(), &self.internal_rpc);
        if let Some(resolver) = connection_resolver {
            resolver.reject(error.clone());
        }
    }

    pub(crate) fn finish_connection_shutdown(&self) {
        self.connection_killswitch.kill();
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for Channels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Channels");
        if let Ok(inner) = self.inner.try_read() {
            debug
                .field("channels", &inner.channels.values())
                .field("channel_id", &inner.channel_id)
                .field("configuration", &inner.configuration);
        }
        debug
            .field("frames", &self.frames)
            .field("connection_status", &self.connection_status)
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
        events_sender: EventsSender,
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
            connection_closer,
            self.recovery_config.clone(),
            events_sender,
        )
    }

    fn create(
        &mut self,
        connection_status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        events_sender: EventsSender,
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
                    events_sender,
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
