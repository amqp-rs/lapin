use crate::{
    connection_closer::ConnectionCloser,
    error_handler::ErrorHandler,
    frames::Frames,
    id_sequence::IdSequence,
    internal_rpc::InternalRPCHandle,
    protocol::{AMQPClass, AMQPError, AMQPHardError},
    recovery_config::RecoveryConfig,
    registry::Registry,
    socket_state::SocketStateHandle,
    topology_internal::ChannelDefinitionInternal,
    types::{ChannelId, Identifier, PayloadSize},
    BasicProperties, Channel, ChannelState, Configuration, ConnectionState, ConnectionStatus,
    Error, ErrorKind, Promise, Result,
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use executor_trait::FullExecutor;
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};
use tracing::{debug, error, level_enabled, trace, Level};

#[derive(Clone)]
pub(crate) struct Channels {
    inner: Arc<Mutex<Inner>>,
    connection_status: ConnectionStatus,
    global_registry: Registry,
    internal_rpc: InternalRPCHandle,
    executor: Arc<dyn FullExecutor + Send + Sync>,
    frames: Frames,
    error_handler: ErrorHandler,
}

impl Channels {
    pub(crate) fn new(
        configuration: Configuration,
        connection_status: ConnectionStatus,
        global_registry: Registry,
        waker: SocketStateHandle,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        recovery_config: RecoveryConfig,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::new(
                configuration,
                waker,
                recovery_config,
            ))),
            connection_status,
            global_registry,
            internal_rpc,
            executor,
            frames,
            error_handler: ErrorHandler::default(),
        }
    }

    pub(crate) fn create(&self, connection_closer: Arc<ConnectionCloser>) -> Result<Channel> {
        self.lock_inner().create(
            self.connection_status.clone(),
            self.global_registry.clone(),
            self.internal_rpc.clone(),
            self.frames.clone(),
            self.executor.clone(),
            connection_closer,
        )
    }

    pub(crate) fn create_zero(&self) {
        self.lock_inner()
            .create_channel(
                0,
                self.connection_status.clone(),
                self.global_registry.clone(),
                self.internal_rpc.clone(),
                self.frames.clone(),
                self.executor.clone(),
                None,
            )
            .set_state(ChannelState::Connected);
    }

    pub(crate) fn get(&self, id: ChannelId) -> Option<Channel> {
        self.lock_inner().channels.get(&id).cloned()
    }

    pub(crate) fn remove(&self, id: ChannelId, error: Error) -> Result<()> {
        self.frames.clear_expected_replies(id, error);
        if self.lock_inner().channels.remove(&id).is_some() {
            Ok(())
        } else {
            Err(ErrorKind::InvalidChannel(id).into())
        }
    }

    pub(crate) fn receive_method(&self, id: ChannelId, method: AMQPClass) -> Result<()> {
        self.get(id)
            .map(|channel| channel.receive_method(method))
            .unwrap_or_else(|| Err(ErrorKind::InvalidChannel(id).into()))
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

        if let Some(channel0) = self.get(0) {
            let (promise, resolver) = Promise::new();

            if level_enabled!(Level::TRACE) {
                promise.set_marker("Heartbeat".into());
            }

            channel0.send_frame(AMQPFrame::Heartbeat(0), resolver, None);
            self.internal_rpc.register_internal_future(promise);
        }
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
                        format!("heartbeat frame received on channel {}", channel_id).into(),
                    );
                    if let Some(channel0) = self.get(0) {
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
                        format!("content header frame received on channel {}", channel_id).into(),
                    );
                    if let Some(channel0) = self.get(0) {
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

    pub(crate) fn topology(&self) -> Vec<ChannelDefinitionInternal> {
        self.lock_inner()
            .channels
            .values()
            .filter(|c| c.id() != 0)
            .map(Channel::topology)
            .collect()
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
        global_registry: Registry,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        connection_closer: Option<Arc<ConnectionCloser>>,
    ) -> Channel {
        debug!(%id, "create channel");
        let channel = Channel::new(
            id,
            self.configuration.clone(),
            connection_status,
            global_registry,
            self.waker.clone(),
            internal_rpc,
            frames,
            executor,
            connection_closer,
            self.recovery_config.clone(),
        );
        self.channels.insert(id, channel.clone_internal());
        channel
    }

    fn create(
        &mut self,
        connection_status: ConnectionStatus,
        global_registry: Registry,
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
                return Ok(self.create_channel(
                    id,
                    connection_status,
                    global_registry,
                    internal_rpc,
                    frames,
                    executor,
                    Some(connection_closer),
                ));
            }
            id = self.channel_id.next();
        }
        Err(ErrorKind::ChannelsLimitReached.into())
    }
}
