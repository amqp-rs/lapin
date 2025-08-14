use crate::{
    ConnectionProperties, Error, ErrorKind, Promise, Result,
    channel::Channel,
    channels::Channels,
    configuration::Configuration,
    connection_closer::ConnectionCloser,
    connection_status::ConnectionStatus,
    frames::Frames,
    heartbeat::Heartbeat,
    internal_rpc::{InternalRPC, InternalRPCHandle},
    io_loop::IoLoop,
    runtime,
    socket_state::SocketState,
    tcp::{AMQPUriTcpExt, HandshakeResult, OwnedTLSConfig},
    thread::ThreadHandle,
    types::ReplyCode,
    uri::AMQPUri,
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use async_trait::async_trait;
use std::{fmt, io, sync::Arc};
use tracing::{Level, level_enabled, trace};

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
    channels: Channels,
    io_loop: ThreadHandle,
    closer: Arc<ConnectionCloser>,
}

impl Connection {
    fn new(
        configuration: Configuration,
        status: ConnectionStatus,
        channels: Channels,
        internal_rpc: InternalRPCHandle,
    ) -> Self {
        let closer = Arc::new(ConnectionCloser::new(status.clone(), internal_rpc));
        Self {
            configuration,
            status,
            channels,
            io_loop: ThreadHandle::default(),
            closer,
        }
    }

    pub(crate) fn for_reconnect(
        configuration: Configuration,
        status: ConnectionStatus,
        channels: Channels,
        internal_rpc: InternalRPCHandle,
    ) -> Self {
        let conn = Self::new(configuration, status, channels, internal_rpc);
        conn.closer.noop();
        conn
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
            return Err(ErrorKind::InvalidConnectionState(self.status.state()).into());
        }
        let channel = self.channels.create(self.closer.clone())?;
        // FIXME: make sure we have a notifier on error+reconnect
        channel.clone().channel_open(channel).await
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

    /// Request a connection close.
    ///
    /// This method is only successful if the connection is in the connected state,
    /// otherwise an [`InvalidConnectionState`] error is returned.
    ///
    /// [`InvalidConnectionState`]: ./enum.Error.html#variant.InvalidConnectionState
    pub async fn close(&self, reply_code: ReplyCode, reply_text: &str) -> Result<()> {
        if !self.status.connected() {
            return Err(ErrorKind::InvalidConnectionState(self.status.state()).into());
        }

        self.channels.set_connection_closing();
        self.channels
            .channel0()
            .connection_close(reply_code, reply_text, 0, 0)
            .await
    }

    /// Block all consumers and publishers on this connection
    pub async fn block(&self, reason: &str) -> Result<()> {
        self.channels.channel0().connection_blocked(reason).await
    }

    /// Unblock all consumers and publishers on this connection
    pub async fn unblock(&self) -> Result<()> {
        self.channels.channel0().connection_unblocked().await
    }

    /// Update the secret used by some authentication module such as OAuth2
    pub async fn update_secret(&self, new_secret: &str, reason: &str) -> Result<()> {
        self.channels
            .channel0()
            .connection_update_secret(new_secret, reason)
            .await
    }

    pub async fn connector(
        uri: AMQPUri,
        connect: Box<
            dyn (Fn(AMQPUri) -> Box<dyn Future<Output = HandshakeResult> + Send + Sync>)
                + Send
                + Sync,
        >,
        options: ConnectionProperties,
    ) -> Result<Connection> {
        let executor = runtime::executor()?;
        let reactor = runtime::reactor()?;
        let configuration = Configuration::new(&uri);
        let status = ConnectionStatus::new(&uri);
        let frames = Frames::default();
        let socket_state = SocketState::default();
        let internal_rpc = InternalRPC::new(executor.clone(), socket_state.handle());
        let heartbeat = Heartbeat::new(status.clone(), executor.clone(), reactor.clone());
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            socket_state.handle(),
            internal_rpc.handle(),
            frames.clone(),
            heartbeat.clone(),
            executor,
            uri.clone(),
            options.clone(),
        );
        let conn = Connection::new(configuration, status, channels, internal_rpc.handle());
        let io_loop = IoLoop::new(
            conn.status.clone(),
            conn.configuration.clone(),
            conn.channels.clone(),
            internal_rpc.handle(),
            frames,
            socket_state,
            connect.into(),
            uri.clone(),
            heartbeat,
        );

        internal_rpc.start(conn.channels.clone());
        conn.io_loop.register(io_loop.start(reactor)?);
        conn.start(uri, options).await
    }

    pub(crate) async fn start(
        self,
        uri: AMQPUri,
        options: ConnectionProperties,
    ) -> Result<Connection> {
        let (promise_out, resolver_out) = Promise::new();
        let (promise_in, resolver_in) = Promise::new();
        if level_enabled!(Level::TRACE) {
            promise_out.set_marker("ProtocolHeader".into());
            promise_in.set_marker("ProtocolHeader.Ok".into());
        }
        let channel0 = self.channels.channel0();

        trace!("Set connection as connecting");
        self.status.clone().set_connecting(
            resolver_out.clone(),
            resolver_in,
            self,
            uri.authority.userinfo.into(),
            uri.query.auth_mechanism.unwrap_or_default(),
            options,
        )?;

        trace!("Sending protocol header to server");
        channel0.send_frame(
            AMQPFrame::ProtocolHeader(ProtocolVersion::amqp_0_9_1()),
            resolver_out,
            None,
        );

        promise_out.await?;
        trace!("Sent protocol header to server, waiting for connection flow");
        promise_in.await
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
        let config = Arc::new(config);
        Connection::connector(
            self,
            Box::new(move |uri| {
                let config = config.clone();
                Box::new(
                    async move { AMQPUriTcpExt::connect_with_config(&uri, (*config).as_ref()) },
                )
            }),
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
            Err(err) => Err(io::Error::other(err).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BasicProperties;
    use crate::channel_receiver_state::{ChannelReceiverState, DeliveryCause};
    use crate::channel_status::ChannelState;
    use crate::connection_status::ConnectionState;
    use crate::options::BasicConsumeOptions;
    use crate::types::{ChannelId, FieldTable, ShortString};
    use amq_protocol::frame::AMQPContentHeader;
    use amq_protocol::protocol::{AMQPClass, basic};
    use executor_trait::FullExecutor;

    fn create_connection(executor: Arc<dyn FullExecutor + Send + Sync>) -> Connection {
        let uri = AMQPUri::default();
        let reactor = Arc::new(async_reactor_trait::AsyncIo);
        let configuration = Configuration::new(&uri);
        let status = ConnectionStatus::new(&uri);
        let frames = Frames::default();
        let socket_state = SocketState::default();
        let internal_rpc = InternalRPC::new(executor.clone(), socket_state.handle());
        let heartbeat = Heartbeat::new(status.clone(), executor.clone(), reactor);
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            socket_state.handle(),
            internal_rpc.handle(),
            frames.clone(),
            heartbeat.clone(),
            executor,
            uri.clone(),
            ConnectionProperties::default(),
        );
        let conn = Connection::new(configuration, status, channels, internal_rpc.handle());
        conn.status.set_state(ConnectionState::Connected);
        conn
    }

    #[test]
    fn channel_limit() {
        let _ = tracing_subscriber::fmt::try_init();

        // Bootstrap connection state to a consuming state
        let executor = Arc::new(async_global_executor_trait::AsyncGlobalExecutor);
        let conn = create_connection(executor.clone());
        conn.configuration.set_channel_max(ChannelId::MAX);
        for _ in 1..=ChannelId::MAX {
            conn.channels.create(conn.closer.clone()).unwrap();
        }

        assert_eq!(
            conn.channels.create(conn.closer.clone()),
            Err(ErrorKind::ChannelsLimitReached.into())
        );
    }

    #[test]
    fn basic_consume_small_payload() {
        let _ = tracing_subscriber::fmt::try_init();

        use crate::consumer::Consumer;

        // Bootstrap connection state to a consuming state
        let executor = Arc::new(async_global_executor_trait::AsyncGlobalExecutor);
        let conn = create_connection(executor.clone());
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
        let executor = Arc::new(async_global_executor_trait::AsyncGlobalExecutor);
        let conn = create_connection(executor.clone());
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
