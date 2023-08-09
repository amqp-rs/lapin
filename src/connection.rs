use crate::{
    channel::Channel,
    channels::Channels,
    configuration::Configuration,
    connection_closer::ConnectionCloser,
    connection_properties::ConnectionProperties,
    connection_status::{ConnectionState, ConnectionStatus, ConnectionStep},
    frames::Frames,
    heartbeat::Heartbeat,
    internal_rpc::{InternalRPC, InternalRPCHandle},
    io_loop::IoLoop,
    options::{ExchangeBindOptions, QueueBindOptions},
    registry::Registry,
    socket_state::{SocketState, SocketStateHandle},
    tcp::{AMQPUriTcpExt, HandshakeResult, OwnedTLSConfig},
    thread::ThreadHandle,
    topology::{RestoredChannel, RestoredTopology, TopologyDefinition},
    topology_internal::TopologyInternal,
    types::ReplyCode,
    uri::AMQPUri,
    Error, Promise, Result, TcpStream,
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use async_trait::async_trait;
use executor_trait::FullExecutor;
use reactor_trait::IOHandle;
use std::{fmt, io, sync::Arc};
use tracing::{level_enabled, Level};

/// A TCP connection to the AMQP server.
///
/// To connect to the server, one of the [`connect`] methods has to be called.
///
/// Afterwards, create a [`Channel`] by calling [`create_channel`].
///
/// Also see the RabbitMQ documentation on [connections](https://www.rabbitmq.com/connections.html).
///
/// [`connect`]: ./struct.Connection.html#method.connect
/// [`Channel`]: ./struct.Channel.html
/// [`create_channel`]: ./struct.Connection.html#method.create_channel
pub struct Connection {
    configuration: Configuration,
    status: ConnectionStatus,
    global_registry: Registry,
    channels: Channels,
    io_loop: ThreadHandle,
    closer: Arc<ConnectionCloser>,
}

impl Connection {
    fn new(
        waker: SocketStateHandle,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn FullExecutor + Send + Sync>,
    ) -> Self {
        let configuration = Configuration::default();
        let status = ConnectionStatus::default();
        let global_registry = Registry::default();
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            global_registry.clone(),
            waker,
            internal_rpc.clone(),
            frames,
            executor,
        );
        let closer = Arc::new(ConnectionCloser::new(status.clone(), internal_rpc));
        let connection = Self {
            configuration,
            status,
            global_registry,
            channels,
            io_loop: ThreadHandle::default(),
            closer,
        };

        connection.channels.create_zero();
        connection
    }

    /// Connect to an AMQP Server.
    ///
    /// The URI must be in the following format:
    ///
    /// * `amqp://127.0.0.1:5672` will connect to the default virtual host `/`.
    /// * `amqp://127.0.0.1:5672/` will connect to the virtual host `""` (empty string).
    /// * `amqp://127.0.0.1:5672/%2f` will connect to the default virtual host `/`.
    ///
    /// Note that the virtual host has to be escaped with
    /// [URL encoding](https://en.wikipedia.org/wiki/Percent-encoding).
    pub async fn connect(uri: &str, options: ConnectionProperties) -> Result<Connection> {
        Connect::connect(uri, options, OwnedTLSConfig::default()).await
    }

    /// Connect to an AMQP Server.
    pub async fn connect_with_config(
        uri: &str,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
    ) -> Result<Connection> {
        Connect::connect(uri, options, config).await
    }

    /// Connect to an AMQP Server.
    pub async fn connect_uri(uri: AMQPUri, options: ConnectionProperties) -> Result<Connection> {
        Connect::connect(uri, options, OwnedTLSConfig::default()).await
    }

    /// Connect to an AMQP Server
    pub async fn connect_uri_with_config(
        uri: AMQPUri,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
    ) -> Result<Connection> {
        Connect::connect(uri, options, config).await
    }

    /// Creates a new [`Channel`] on this connection.
    ///
    /// This method is only successful if the client is connected.
    /// Otherwise, [`InvalidConnectionState`] error is returned.
    ///
    /// [`Channel`]: ./struct.Channel.html
    /// [`InvalidConnectionState`]: ./enum.Error.html#variant.InvalidConnectionState
    pub async fn create_channel(&self) -> Result<Channel> {
        if !self.status.connected() {
            return Err(Error::InvalidConnectionState(self.status.state()));
        }
        let channel = self.channels.create(self.closer.clone())?;
        channel.clone().channel_open(channel).await
    }

    /// Restore the specified topology
    pub async fn restore(&self, topology: TopologyDefinition) -> Result<RestoredTopology> {
        self.restore_internal(topology.into()).await
    }

    pub(crate) async fn restore_internal(
        &self,
        topology: TopologyInternal,
    ) -> Result<RestoredTopology> {
        let mut restored = RestoredTopology::default();

        // First, recreate all channels
        for c in &topology.channels {
            restored
                .channels
                .push(RestoredChannel::new(if let Some(c) = c.channel.clone() {
                    let channel = c.clone();
                    c.reset();
                    c.channel_open(channel).await?
                } else {
                    self.create_channel().await?
                }));
        }

        // Then, ensure we have at least one channel to restore everything else
        let channel = if let Some(chan) = restored.channels.get(0) {
            chan.channel.clone()
        } else {
            self.create_channel().await?
        };

        // First, redeclare all exchanges
        for ex in &topology.exchanges {
            channel
                .exchange_declare(
                    ex.name.as_str(),
                    ex.kind.clone().unwrap_or_default(),
                    ex.options.unwrap_or_default(),
                    ex.arguments.clone().unwrap_or_default(),
                )
                .await?;
        }

        // Second, redeclare all exchange bindings
        for ex in &topology.exchanges {
            for binding in &ex.bindings {
                channel
                    .exchange_bind(
                        ex.name.as_str(),
                        binding.source.as_str(),
                        binding.routing_key.as_str(),
                        ExchangeBindOptions::default(),
                        binding.arguments.clone(),
                    )
                    .await?;
            }
        }

        // Third, redeclare all "global" (e.g. non exclusive) queues
        for queue in &topology.queues {
            if queue.is_declared() {
                restored.queues.push(
                    channel
                        .queue_declare(
                            queue.name.as_str(),
                            queue.options.unwrap_or_default(),
                            queue.arguments.clone().unwrap_or_default(),
                        )
                        .await?,
                );
            }
        }

        // Fourth, redeclare all global queues bindings
        for queue in &topology.queues {
            for binding in &queue.bindings {
                channel
                    .queue_bind(
                        queue.name.as_str(),
                        binding.source.as_str(),
                        binding.routing_key.as_str(),
                        QueueBindOptions::default(),
                        binding.arguments.clone(),
                    )
                    .await?;
            }
        }

        // Fifth, restore all channel-specific queues/bindings/consumers
        for (n, ch) in topology.channels.iter().enumerate() {
            let c = &mut restored.channels[n];
            c.channel.clone().restore(ch, c).await?;
        }
        Ok(restored)
    }

    /// Block current thread while the connection is still active.
    /// This is useful when you only have a consumer and nothing else keeping your application
    /// "alive".
    pub fn run(self) -> Result<()> {
        let io_loop = self.io_loop.clone();
        drop(self);
        io_loop.wait("io loop")
    }

    pub fn on_error<E: FnMut(Error) + Send + 'static>(&self, handler: E) {
        self.channels.set_error_handler(handler);
    }

    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    pub async fn close(&self, reply_code: ReplyCode, reply_text: &str) -> Result<()> {
        self.channels.set_connection_closing();
        if let Some(channel0) = self.channels.get(0) {
            channel0
                .connection_close(reply_code, reply_text, 0, 0)
                .await
        } else {
            Ok(())
        }
    }

    /// Block all consumers and publishers on this connection
    pub async fn block(&self, reason: &str) -> Result<()> {
        if let Some(channel0) = self.channels.get(0) {
            channel0.connection_blocked(reason).await
        } else {
            Err(Error::InvalidConnectionState(self.status.state()))
        }
    }

    /// Unblock all consumers and publishers on this connection
    pub async fn unblock(&self) -> Result<()> {
        if let Some(channel0) = self.channels.get(0) {
            channel0.connection_unblocked().await
        } else {
            Err(Error::InvalidConnectionState(self.status.state()))
        }
    }

    /// Update the secret used by some authentication module such as OAuth2
    pub async fn update_secret(&self, new_secret: &str, reason: &str) -> Result<()> {
        if let Some(channel0) = self.channels.get(0) {
            channel0.connection_update_secret(new_secret, reason).await
        } else {
            Err(Error::InvalidConnectionState(self.status.state()))
        }
    }

    pub async fn connector(
        uri: AMQPUri,
        connect: Box<dyn FnOnce(&AMQPUri) -> HandshakeResult + Send + Sync>,
        mut options: ConnectionProperties,
    ) -> Result<Connection> {
        let executor = options.executor.take();

        #[cfg(feature = "default-runtime")]
        let executor =
            executor.or_else(|| Some(Arc::new(async_global_executor_trait::AsyncGlobalExecutor)));

        let executor = executor
            .expect("executor should be provided with no default executor feature was enabled");

        let (connect_promise, resolver) = pinky_swear::PinkySwear::<Result<TcpStream>>::new();
        let connect_uri = uri.clone();
        executor.spawn({
            let executor = executor.clone();
            Box::pin(async move {
                executor
                    .spawn_blocking(Box::new(move || {
                        let mut res = connect(&connect_uri);
                        loop {
                            match res {
                                Ok(stream) => {
                                    resolver.swear(Ok(stream));
                                    break;
                                }
                                Err(mid) => match mid.into_mid_handshake_tls_stream() {
                                    Err(err) => {
                                        resolver.swear(Err(err.into()));
                                        break;
                                    }
                                    Ok(mid) => {
                                        res = mid.handshake();
                                    }
                                },
                            }
                        }
                    }))
                    .await;
            })
        });

        let reactor = options.reactor.take();

        #[cfg(feature = "default-runtime")]
        let reactor = reactor.or_else(|| Some(Arc::new(async_reactor_trait::AsyncIo)));

        let reactor = reactor
            .expect("reactor should be provided with no default reactor feature was enabled");

        let socket_state = SocketState::default();
        let waker = socket_state.handle();
        let internal_rpc = InternalRPC::new(executor.clone(), waker.clone());
        let frames = Frames::default();
        let conn = Connection::new(
            waker,
            internal_rpc.handle(),
            frames.clone(),
            executor.clone(),
        );
        let status = conn.status.clone();
        let configuration = conn.configuration.clone();
        status.set_vhost(&uri.vhost);
        status.set_username(&uri.authority.userinfo.username);
        if let Some(frame_max) = uri.query.frame_max {
            configuration.set_frame_max(frame_max);
        }
        if let Some(channel_max) = uri.query.channel_max {
            configuration.set_channel_max(channel_max);
        }
        if let Some(heartbeat) = uri.query.heartbeat {
            configuration.set_heartbeat(heartbeat);
        }
        let (promise_out, resolver) = Promise::new();
        if level_enabled!(Level::TRACE) {
            promise_out.set_marker("ProtocolHeader".into());
        }
        let channels = conn.channels.clone();
        if let Some(channel0) = channels.get(0) {
            channel0.send_frame(
                AMQPFrame::ProtocolHeader(ProtocolVersion::amqp_0_9_1()),
                resolver,
                None,
            )
        };
        let (promise_in, resolver) = Promise::new();
        if level_enabled!(Level::TRACE) {
            promise_in.set_marker("ProtocolHeader.Ok".into());
        }
        let io_loop_handle = conn.io_loop.clone();
        status.set_state(ConnectionState::Connecting);
        status.set_connection_step(ConnectionStep::ProtocolHeader(
            resolver,
            conn,
            uri.authority.userinfo.into(),
            uri.query.auth_mechanism.unwrap_or_default(),
            options,
        ));
        let stream = connect_promise
            .await
            .and_then(|stream| reactor.register(IOHandle::new(stream)).map_err(Into::into))
            .map_err(|error| {
                // We don't actually need the resolver as we already pass it around to the failing
                // code which will propagate the error. We only want to flush the status internal
                // state.
                let _ = status.connection_resolver();
                error
            })?
            .into();
        let heartbeat = Heartbeat::new(channels.clone(), executor.clone(), reactor);
        let internal_rpc_handle = internal_rpc.handle();
        executor.spawn(Box::pin(internal_rpc.run(channels.clone())));
        IoLoop::new(
            status,
            configuration,
            channels,
            internal_rpc_handle,
            frames,
            socket_state,
            io_loop_handle,
            stream,
            heartbeat,
        )
        .await
        .and_then(IoLoop::start)?;
        promise_out.await?;
        promise_in.await
    }

    /// Get the current topology
    ///
    /// This includes exchanges, queues, bindings and consumers declared by this Connection
    pub fn topology(&self) -> TopologyDefinition {
        self.topology_internal().into()
    }

    pub(crate) fn topology_internal(&self) -> TopologyInternal {
        TopologyInternal {
            exchanges: self.global_registry.exchanges_topology(),
            queues: self.global_registry.queues_topology(false),
            channels: self.channels.topology(),
        }
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("configuration", &self.configuration)
            .field("status", &self.status)
            .field("channels", &self.channels)
            .finish()
    }
}

/// Trait providing a method to connect to an AMQP server
#[async_trait]
pub trait Connect {
    /// connect to an AMQP server
    async fn connect(
        self,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
    ) -> Result<Connection>;
}

#[async_trait]
impl Connect for AMQPUri {
    async fn connect(
        self,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
    ) -> Result<Connection> {
        Connection::connector(
            self,
            Box::new(move |uri| AMQPUriTcpExt::connect_with_config(uri, config.as_ref())),
            options,
        )
        .await
    }
}

#[async_trait]
impl Connect for &str {
    async fn connect(
        self,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
    ) -> Result<Connection> {
        match self.parse::<AMQPUri>() {
            Ok(uri) => Connect::connect(uri, options, config).await,
            Err(err) => Err(io::Error::new(io::ErrorKind::Other, err).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel_receiver_state::{ChannelReceiverState, DeliveryCause};
    use crate::channel_status::ChannelState;
    use crate::options::BasicConsumeOptions;
    use crate::types::{FieldTable, ShortString};
    use crate::BasicProperties;
    use amq_protocol::frame::AMQPContentHeader;
    use amq_protocol::protocol::{basic, AMQPClass};

    #[test]
    fn basic_consume_small_payload() {
        let _ = tracing_subscriber::fmt::try_init();

        use crate::consumer::Consumer;

        // Bootstrap connection state to a consuming state
        let executor = Arc::new(async_global_executor_trait::AsyncGlobalExecutor);
        let socket_state = SocketState::default();
        let waker = socket_state.handle();
        let internal_rpc = InternalRPC::new(executor.clone(), waker.clone());
        let conn = Connection::new(
            waker,
            internal_rpc.handle(),
            Frames::default(),
            executor.clone(),
        );
        conn.status.set_state(ConnectionState::Connected);
        conn.configuration.set_channel_max(2047);
        let channel = conn.channels.create(conn.closer.clone()).unwrap();
        channel.set_state(ChannelState::Connected);
        let queue_name = ShortString::from("consumed");
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(
            consumer_tag.clone(),
            executor,
            None,
            queue_name.clone(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        );
        if let Some(c) = conn.channels.get(channel.id()) {
            c.register_consumer(consumer_tag.clone(), consumer);
            c.register_queue(queue_name.clone(), Default::default(), Default::default());
        }
        // Now test the state machine behaviour
        {
            let method = AMQPClass::Basic(basic::AMQPMethod::Deliver(basic::Deliver {
                consumer_tag: consumer_tag.clone(),
                delivery_tag: 1,
                redelivered: false,
                exchange: "".into(),
                routing_key: queue_name,
            }));
            let class_id = method.get_amqp_class_id();
            let deliver_frame = AMQPFrame::Method(channel.id(), method);
            conn.channels.handle_frame(deliver_frame).unwrap();
            let channel_state = channel.status().receiver_state();
            let expected_state = ChannelReceiverState::WillReceiveContent(
                class_id,
                DeliveryCause::Consume(consumer_tag.clone()),
            );
            assert_eq!(channel_state, expected_state);
        }
        {
            let header_frame = AMQPFrame::Header(
                channel.id(),
                60,
                Box::new(AMQPContentHeader {
                    class_id: 60,
                    body_size: 2,
                    properties: BasicProperties::default(),
                }),
            );
            conn.channels.handle_frame(header_frame).unwrap();
            let channel_state = channel.status().receiver_state();
            let expected_state =
                ChannelReceiverState::ReceivingContent(DeliveryCause::Consume(consumer_tag), 2);
            assert_eq!(channel_state, expected_state);
        }
        {
            let body_frame = AMQPFrame::Body(channel.id(), b"{}".to_vec());
            conn.channels.handle_frame(body_frame).unwrap();
            let channel_state = channel.status().state();
            let expected_state = ChannelState::Connected;
            assert_eq!(channel_state, expected_state);
        }
    }

    #[test]
    fn basic_consume_empty_payload() {
        let _ = tracing_subscriber::fmt::try_init();

        use crate::consumer::Consumer;

        // Bootstrap connection state to a consuming state
        let socket_state = SocketState::default();
        let waker = socket_state.handle();
        let executor = Arc::new(async_global_executor_trait::AsyncGlobalExecutor);
        let internal_rpc = InternalRPC::new(executor.clone(), waker.clone());
        let conn = Connection::new(
            waker,
            internal_rpc.handle(),
            Frames::default(),
            executor.clone(),
        );
        conn.status.set_state(ConnectionState::Connected);
        conn.configuration.set_channel_max(2047);
        let channel = conn.channels.create(conn.closer.clone()).unwrap();
        channel.set_state(ChannelState::Connected);
        let queue_name = ShortString::from("consumed");
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(
            consumer_tag.clone(),
            executor,
            None,
            queue_name.clone(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        );
        if let Some(c) = conn.channels.get(channel.id()) {
            c.register_consumer(consumer_tag.clone(), consumer);
            c.register_queue(queue_name.clone(), Default::default(), Default::default());
        }
        // Now test the state machine behaviour
        {
            let method = AMQPClass::Basic(basic::AMQPMethod::Deliver(basic::Deliver {
                consumer_tag: consumer_tag.clone(),
                delivery_tag: 1,
                redelivered: false,
                exchange: "".into(),
                routing_key: queue_name,
            }));
            let class_id = method.get_amqp_class_id();
            let deliver_frame = AMQPFrame::Method(channel.id(), method);
            conn.channels.handle_frame(deliver_frame).unwrap();
            let channel_state = channel.status().receiver_state();
            let expected_state = ChannelReceiverState::WillReceiveContent(
                class_id,
                DeliveryCause::Consume(consumer_tag),
            );
            assert_eq!(channel_state, expected_state);
        }
        {
            let header_frame = AMQPFrame::Header(
                channel.id(),
                60,
                Box::new(AMQPContentHeader {
                    class_id: 60,
                    body_size: 0,
                    properties: BasicProperties::default(),
                }),
            );
            conn.channels.handle_frame(header_frame).unwrap();
            let channel_state = channel.status().state();
            let expected_state = ChannelState::Connected;
            assert_eq!(channel_state, expected_state);
        }
    }
}
