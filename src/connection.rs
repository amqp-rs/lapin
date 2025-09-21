use crate::{
    AsyncTcpStream, ConnectionProperties, ConnectionStatus, Event, Promise, Result,
    channel::Channel,
    channels::Channels,
    configuration::Configuration,
    connection_closer::ConnectionCloser,
    events::Events,
    frames::Frames,
    heartbeat::Heartbeat,
    internal_rpc::{InternalRPC, InternalRPCHandle},
    io_loop::IoLoop,
    runtime,
    secret_update::SecretUpdate,
    socket_state::SocketState,
    tcp::{AMQPUriTcpExt, OwnedTLSConfig},
    thread::ThreadHandle,
    types::{LongString, ReplyCode, ShortString},
    uri::AMQPUri,
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use async_rs::{Runtime, traits::*};
use async_trait::async_trait;
use futures_core::Stream;
use std::{fmt, io, sync::Arc};
use tracing::trace;

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
    internal_rpc: InternalRPCHandle,
    events: Events,
    io_loop: ThreadHandle,
    closer: Arc<ConnectionCloser>,
}

impl Connection {
    fn new(
        configuration: Configuration,
        status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        events: Events,
    ) -> Self {
        let closer = Arc::new(ConnectionCloser::new(status.clone(), internal_rpc.clone()));
        Self {
            configuration,
            status,
            internal_rpc,
            events,
            io_loop: ThreadHandle::default(),
            closer,
        }
    }

    pub(crate) fn for_reconnect(
        configuration: Configuration,
        status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        events: Events,
    ) -> Self {
        let conn = Self::new(configuration, status, internal_rpc, events);
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
    pub async fn connect(uri: &str, options: ConnectionProperties) -> Result<Self> {
        Connect::connect(uri, options).await
    }

    /// Connect to an AMQP Server.
    pub async fn connect_with_runtime<RK: RuntimeKit + Send + Sync + Clone + 'static>(
        uri: &str,
        options: ConnectionProperties,
        runtime: Runtime<RK>,
    ) -> Result<Self> {
        uri.connect_with_config(options, OwnedTLSConfig::default(), runtime)
            .await
    }

    /// Connect to an AMQP Server.
    pub async fn connect_with_config<RK: RuntimeKit + Send + Sync + Clone + 'static>(
        uri: &str,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
        runtime: Runtime<RK>,
    ) -> Result<Self> {
        uri.connect_with_config(options, config, runtime).await
    }

    /// Connect to an AMQP Server.
    pub async fn connect_uri(uri: AMQPUri, options: ConnectionProperties) -> Result<Self> {
        Connect::connect(uri, options).await
    }

    /// Connect to an AMQP Server
    pub async fn connect_uri_with_runtime<RK: RuntimeKit + Send + Sync + Clone + 'static>(
        uri: AMQPUri,
        options: ConnectionProperties,
        runtime: Runtime<RK>,
    ) -> Result<Self> {
        uri.connect_with_config(options, OwnedTLSConfig::default(), runtime)
            .await
    }

    /// Connect to an AMQP Server
    pub async fn connect_uri_with_config<RK: RuntimeKit + Send + Sync + Clone + 'static>(
        uri: AMQPUri,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
        runtime: Runtime<RK>,
    ) -> Result<Self> {
        uri.connect_with_config(options, config, runtime).await
    }

    /// Creates a new [`Channel`] on this connection.
    ///
    /// This method is only successful if the client is connected.
    /// Otherwise, [`InvalidConnectionState`] error is returned.
    ///
    /// [`Channel`]: ./struct.Channel.html
    /// [`InvalidConnectionState`]: ./enum.Error.html#variant.InvalidConnectionState
    pub async fn create_channel(&self) -> Result<Channel> {
        self.status.ensure_connected()?;
        self.internal_rpc.create_channel(self.closer.clone()).await
    }

    /// Get a Stream of connection Events
    pub fn events_listener(&self) -> impl Stream<Item = Event> + Send + 'static {
        self.events.listener()
    }

    /// Block current thread while the connection is still active.
    /// This is useful when you only have a consumer and nothing else keeping your application
    /// "alive".
    pub fn run(self) -> Result<()> {
        let io_loop = self.io_loop.clone();
        drop(self);
        io_loop.wait("io loop")
    }

    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub(crate) fn configuration_mut(&mut self) -> &mut Configuration {
        &mut self.configuration
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
    pub async fn close(&self, reply_code: ReplyCode, reply_text: ShortString) -> Result<()> {
        self.status.ensure_connected()?;
        self.internal_rpc
            .close_connection_checked(reply_code, reply_text, 0, 0)
            .await
    }

    /// Update the secret used by some authentication module such as OAuth2
    pub async fn update_secret(&self, new_secret: LongString, reason: ShortString) -> Result<()> {
        self.status.ensure_connected()?;
        self.internal_rpc.update_secret(new_secret, reason).await
    }

    pub async fn connector<RK: RuntimeKit + Clone + Send + 'static>(
        uri: AMQPUri,
        runtime: Runtime<RK>,
        connect: impl AsyncFn(
            AMQPUri,
            Runtime<RK>,
        ) -> Result<AsyncTcpStream<<RK as Reactor>::TcpStream>>
        + Send
        + Sync
        + 'static,
        options: ConnectionProperties,
    ) -> Result<Self> {
        let configuration = Configuration::new(&uri, options);
        let status = ConnectionStatus::new(&uri);
        let frames = Frames::default();
        let socket_state = SocketState::default();
        let heartbeat = Heartbeat::new(status.clone(), runtime.clone());
        let secret_update = SecretUpdate::new(
            status.clone(),
            runtime.clone(),
            configuration.auth_provider.clone(),
        );
        let internal_rpc = InternalRPC::new(
            runtime.clone(),
            heartbeat.clone(),
            secret_update,
            frames.clone(),
            socket_state.handle(),
        );
        let events = Events::new();
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            socket_state.handle(),
            internal_rpc.handle(),
            frames.clone(),
            events.clone(),
        );
        let channel0 = channels.channel0();
        let conn = Connection::new(configuration, status, internal_rpc.handle(), events);
        let io_loop = IoLoop::new(
            conn.status.clone(),
            conn.configuration.clone(),
            channels.clone(),
            internal_rpc.handle(),
            frames,
            socket_state,
            heartbeat,
            runtime,
            connect,
            uri,
            conn.configuration().backoff,
        );

        internal_rpc.start(channels);
        conn.io_loop.register(io_loop.start()?);
        conn.start(channel0).await
    }

    pub(crate) async fn start(self, channel0: Channel) -> Result<Self> {
        let (promise, resolver) = Promise::new("ProtocolHeader");

        trace!("Set connection as connecting");
        self.status.clone().set_connecting(resolver.clone(), self)?;

        trace!("Sending protocol header to server");
        channel0.send_frame(
            AMQPFrame::ProtocolHeader(ProtocolVersion::amqp_0_9_1()),
            Box::new(resolver),
            None, // FIXME: switch to ExpectedReply instead of connection status?
            None,
        );

        trace!("Sent protocol header to server, waiting for connection flow");
        promise.await
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("configuration", &self.configuration)
            .field("status", &self.status)
            .finish()
    }
}

/// Trait providing a method to connect to an AMQP server
#[async_trait]
pub trait Connect {
    /// connect to an AMQP server
    async fn connect(self, options: ConnectionProperties) -> Result<Connection>
    where
        Self: Sized,
    {
        self.connect_with_config(
            options,
            OwnedTLSConfig::default(),
            runtime::default_runtime()?,
        )
        .await
    }

    /// connect to an AMQP server
    async fn connect_with_config<RK: RuntimeKit + Send + Sync + Clone + 'static>(
        self,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
        runtime: Runtime<RK>,
    ) -> Result<Connection>
    where
        Self: Sized;
}

#[async_trait]
impl Connect for AMQPUri {
    async fn connect_with_config<RK: RuntimeKit + Send + Sync + Clone + 'static>(
        self,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
        runtime: Runtime<RK>,
    ) -> Result<Connection> {
        Connection::connector(
            self,
            runtime,
            async move |uri, runtime| {
                Ok(
                    AMQPUriTcpExt::connect_with_config_async(&uri, config.as_ref(), &runtime)
                        .await?,
                )
            },
            options,
        )
        .await
    }
}

#[async_trait]
impl Connect for &str {
    async fn connect_with_config<RK: RuntimeKit + Send + Sync + Clone + 'static>(
        self,
        options: ConnectionProperties,
        config: OwnedTLSConfig,
        runtime: Runtime<RK>,
    ) -> Result<Connection> {
        match self.parse::<AMQPUri>() {
            Ok(uri) => uri.connect_with_config(options, config, runtime).await,
            Err(err) => Err(io::Error::other(err).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BasicProperties, ChannelState, ConnectionProperties, ConnectionState, ErrorKind,
        channel_receiver_state::{ChannelReceiverState, DeliveryCause},
        options::BasicConsumeOptions,
        secret_update::SecretUpdate,
        types::{ChannelId, FieldTable, ShortString},
    };
    use amq_protocol::{
        frame::AMQPContentHeader,
        protocol::{AMQPClass, basic},
    };

    fn create_connection() -> (Connection, Channels, InternalRPCHandle) {
        let uri = AMQPUri::default();
        let runtime = runtime::default_runtime().unwrap();
        let configuration = Configuration::new(&uri, ConnectionProperties::default());
        let status = ConnectionStatus::new(&uri);
        let frames = Frames::default();
        let socket_state = SocketState::default();
        let heartbeat = Heartbeat::new(status.clone(), runtime.clone());
        let secret_update = SecretUpdate::new(
            status.clone(),
            runtime.clone(),
            configuration.auth_provider.clone(),
        );
        let internal_rpc = InternalRPC::new(
            runtime,
            heartbeat,
            secret_update,
            frames.clone(),
            socket_state.handle(),
        );
        let events = Events::new();
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            socket_state.handle(),
            internal_rpc.handle(),
            frames.clone(),
            events.clone(),
        );
        let conn = Connection::new(configuration, status, internal_rpc.handle(), events);
        conn.status.set_state(ConnectionState::Connected);
        (conn, channels, internal_rpc.handle())
    }

    #[test]
    fn channel_limit() {
        let _ = tracing_subscriber::fmt::try_init();

        // Bootstrap connection state to a consuming state
        let (conn, channels, _) = create_connection();
        conn.configuration.set_channel_max(ChannelId::MAX);
        for _ in 1..=ChannelId::MAX {
            channels.create(conn.closer.clone()).unwrap();
        }

        assert_eq!(
            channels.create(conn.closer.clone()),
            Err(ErrorKind::ChannelsLimitReached.into())
        );
    }

    #[test]
    fn basic_consume_small_payload() {
        let _ = tracing_subscriber::fmt::try_init();

        use crate::consumer::Consumer;

        // Bootstrap connection state to a consuming state
        let (conn, channels, internal_rpc) = create_connection();
        conn.configuration.set_channel_max(2047);
        let channel = channels.create(conn.closer.clone()).unwrap();
        channel.set_state(ChannelState::Connected);
        let queue_name = ShortString::from("consumed");
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(
            consumer_tag.clone(),
            internal_rpc,
            None,
            queue_name.clone(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        );
        if let Some(c) = channels.get(channel.id()) {
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
            channels.handle_frame(deliver_frame).unwrap();
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
                AMQPContentHeader {
                    class_id: 60,
                    body_size: 2,
                    properties: BasicProperties::default(),
                },
            );
            channels.handle_frame(header_frame).unwrap();
            let channel_state = channel.status().receiver_state();
            let expected_state =
                ChannelReceiverState::ReceivingContent(DeliveryCause::Consume(consumer_tag), 2);
            assert_eq!(channel_state, expected_state);
        }
        {
            let body_frame = AMQPFrame::Body(channel.id(), b"{}".to_vec());
            channels.handle_frame(body_frame).unwrap();
            assert!(channel.status().connected());
        }
    }

    #[test]
    fn basic_consume_empty_payload() {
        let _ = tracing_subscriber::fmt::try_init();

        use crate::consumer::Consumer;

        // Bootstrap connection state to a consuming state
        let (conn, channels, internal_rpc) = create_connection();
        conn.configuration.set_channel_max(2047);
        let channel = channels.create(conn.closer.clone()).unwrap();
        channel.set_state(ChannelState::Connected);
        let queue_name = ShortString::from("consumed");
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(
            consumer_tag.clone(),
            internal_rpc,
            None,
            queue_name.clone(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        );
        if let Some(c) = channels.get(channel.id()) {
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
            channels.handle_frame(deliver_frame).unwrap();
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
                AMQPContentHeader {
                    class_id: 60,
                    body_size: 0,
                    properties: BasicProperties::default(),
                },
            );
            channels.handle_frame(header_frame).unwrap();
            assert!(channel.status().connected());
        }
    }
}
