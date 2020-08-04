use crate::{
    connection_closer::ConnectionCloser,
    error_handler::ErrorHandler,
    executor::Executor,
    frames::Frames,
    id_sequence::IdSequence,
    internal_rpc::InternalRPCHandle,
    protocol::{AMQPClass, AMQPError, AMQPHardError},
    socket_state::SocketStateHandle,
    BasicProperties, Channel, ChannelState, Configuration, ConnectionState, ConnectionStatus,
    Error, Promise, Result,
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use log::{debug, error, log_enabled, trace, Level::Trace};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Clone)]
pub(crate) struct Channels {
    inner: Arc<Mutex<Inner>>,
    connection_status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
    executor: Arc<dyn Executor>,
    frames: Frames,
    error_handler: ErrorHandler,
}

impl Channels {
    pub(crate) fn new(
        configuration: Configuration,
        connection_status: ConnectionStatus,
        waker: SocketStateHandle,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn Executor>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::new(configuration, waker))),
            connection_status,
            internal_rpc,
            executor,
            frames,
            error_handler: ErrorHandler::default(),
        }
    }

    pub(crate) fn create(&self, connection_closer: Arc<ConnectionCloser>) -> Result<Channel> {
        self.inner.lock().create(
            self.connection_status.clone(),
            self.internal_rpc.clone(),
            self.frames.clone(),
            self.executor.clone(),
            connection_closer,
        )
    }

    pub(crate) fn create_zero(&self) {
        self.inner
            .lock()
            .create_channel(
                0,
                self.connection_status.clone(),
                self.internal_rpc.clone(),
                self.frames.clone(),
                self.executor.clone(),
                None,
            )
            .set_state(ChannelState::Connected);
    }

    pub(crate) fn get(&self, id: u16) -> Option<Channel> {
        self.inner.lock().channels.get(&id).cloned()
    }

    pub(crate) fn remove(&self, id: u16, error: Error) -> Result<()> {
        self.frames.clear_expected_replies(id, error);
        if self.inner.lock().channels.remove(&id).is_some() {
            Ok(())
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn receive_method(&self, id: u16, method: AMQPClass) -> Result<()> {
        self.get(id)
            .map(|channel| channel.receive_method(method))
            .unwrap_or_else(|| Err(Error::InvalidChannel(id)))
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        id: u16,
        class_id: u16,
        size: u64,
        properties: BasicProperties,
    ) -> Result<()> {
        self.get(id)
            .map(|channel| channel.handle_content_header_frame(class_id, size, properties))
            .unwrap_or_else(|| Err(Error::InvalidChannel(id)))
    }

    pub(crate) fn handle_body_frame(&self, id: u16, payload: Vec<u8>) -> Result<()> {
        self.get(id)
            .map(|channel| channel.handle_body_frame(payload))
            .unwrap_or_else(|| Err(Error::InvalidChannel(id)))
    }

    pub(crate) fn set_connection_closing(&self) {
        self.connection_status.set_state(ConnectionState::Closing);
        for channel in self.inner.lock().channels.values() {
            channel.set_state(ChannelState::Closing);
        }
    }

    pub(crate) fn set_connection_closed(&self, error: Error) -> Result<()> {
        self.connection_status.set_state(ConnectionState::Closed);
        self.inner
            .lock()
            .channels
            .drain()
            .map(|(id, channel)| {
                self.frames.clear_expected_replies(id, error.clone());
                channel.set_state(ChannelState::Closed);
                channel.error_publisher_confirms(error.clone());
                channel.cancel_consumers()
            })
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn set_connection_error(&self, error: Error) -> Result<()> {
        if let ConnectionState::Error = self.connection_status.state() {
            return Ok(());
        }

        error!("Connection error: {}", error);
        self.connection_status.set_state(ConnectionState::Error);
        self.frames.drop_pending(error.clone());
        self.error_handler.on_error(error.clone());
        self.inner
            .lock()
            .channels
            .drain()
            .map(|(id, channel)| {
                self.frames.clear_expected_replies(id, error.clone());
                channel.set_state(ChannelState::Error);
                channel.error_publisher_confirms(error.clone());
                channel.error_consumers(error.clone())
            })
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn flow(&self) -> bool {
        self.inner
            .lock()
            .channels
            .values()
            .all(|c| c.status().flow())
    }

    pub(crate) fn send_heartbeat(&self) -> Result<()> {
        debug!("send heartbeat");

        self.get(0)
            .map(|channel0| {
                let (promise, resolver) = Promise::new();

                if log_enabled!(Trace) {
                    promise.set_marker("Heartbeat".into());
                }

                channel0.send_frame(AMQPFrame::Heartbeat(0), resolver, None);
                self.internal_rpc.register_internal_future(promise)
            })
            .unwrap_or_else(|| {
                Err(Error::InvalidConnectionState(
                    self.connection_status.state(),
                ))
            })
    }

    pub(crate) fn handle_frame(&self, f: AMQPFrame) -> Result<()> {
        if let Err(err) = self.do_handle_frame(f) {
            self.set_connection_error(err.clone())?;
            Err(err)
        } else {
            Ok(())
        }
    }

    fn do_handle_frame(&self, f: AMQPFrame) -> Result<()> {
        trace!("will handle frame: {:?}", f);
        match f {
            AMQPFrame::ProtocolHeader(version) => {
                error!(
                    "we asked for AMQP {} but the server only supports AMQP {}",
                    ProtocolVersion::amqp_0_9_1(),
                    version
                );
                let error = Error::InvalidProtocolVersion(version);
                if let Some(resolver) = self.connection_status.connection_resolver() {
                    resolver.swear(Err(error.clone()));
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
                    error!("received invalid heartbeat on channel {}", channel_id);
                    let error = AMQPError::new(
                        AMQPHardError::FRAMEERROR.into(),
                        format!("heartbeat frame received on channel {}", channel_id).into(),
                    );
                    if let Some(Err(error)) = self.get(0).map(|channel0| {
                        self.internal_rpc
                            .register_internal_future(channel0.connection_close(
                                error.get_id(),
                                error.get_message().as_str(),
                                0,
                                0,
                            ))
                    }) {
                        return Err(error);
                    }
                    return Err(Error::ProtocolError(error));
                }
            }
            AMQPFrame::Header(channel_id, class_id, header) => {
                if channel_id == 0 {
                    error!("received content header on channel {}", channel_id);
                    let error = AMQPError::new(
                        AMQPHardError::CHANNELERROR.into(),
                        format!("content header frame received on channel {}", channel_id).into(),
                    );
                    if let Some(Err(error)) = self.get(0).map(|channel0| {
                        self.internal_rpc
                            .register_internal_future(channel0.connection_close(
                                error.get_id(),
                                error.get_message().as_str(),
                                class_id,
                                0,
                            ))
                    }) {
                        return Err(error);
                    }
                    return Err(Error::ProtocolError(error));
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
}

impl fmt::Debug for Channels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Channels");
        if let Some(inner) = self.inner.try_lock() {
            debug
                .field("channels", &inner.channels.values())
                .field("channel_id", &inner.channel_id)
                .field("configuration", &inner.configuration);
        }
        debug
            .field("frames", &self.frames)
            .field("executor", &self.executor)
            .field("connection_status", &self.connection_status)
            .field("error_handler", &self.error_handler)
            .finish()
    }
}

struct Inner {
    channels: HashMap<u16, Channel>,
    channel_id: IdSequence<u16>,
    configuration: Configuration,
    waker: SocketStateHandle,
}

impl Inner {
    fn new(configuration: Configuration, waker: SocketStateHandle) -> Self {
        Self {
            channels: HashMap::default(),
            channel_id: IdSequence::new(false),
            configuration,
            waker,
        }
    }

    fn create_channel(
        &mut self,
        id: u16,
        connection_status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn Executor>,
        connection_closer: Option<Arc<ConnectionCloser>>,
    ) -> Channel {
        debug!("create channel with id {}", id);
        let channel = Channel::new(
            id,
            self.configuration.clone(),
            connection_status,
            self.waker.clone(),
            internal_rpc,
            frames,
            executor,
            connection_closer,
        );
        self.channels.insert(id, channel.clone_internal());
        channel
    }

    fn create(
        &mut self,
        connection_status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn Executor>,
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
                    internal_rpc,
                    frames,
                    executor,
                    Some(connection_closer),
                ));
            }
            id = self.channel_id.next();
        }
        Err(Error::ChannelsLimitReached)
    }
}
