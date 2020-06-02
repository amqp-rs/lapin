use crate::{
    channel::Channel,
    channels::Channels,
    configuration::Configuration,
    connection_closer::ConnectionCloser,
    connection_properties::ConnectionProperties,
    connection_status::{ConnectionState, ConnectionStatus, ConnectionStep},
    executor::{within_executor, DefaultExecutor, Executor},
    frames::Frames,
    internal_rpc::{InternalRPC, InternalRPCHandle},
    io_loop::IoLoop,
    reactor::DefaultReactorBuilder,
    socket_state::{SocketState, SocketStateHandle},
    tcp::{AMQPUriTcpExt, HandshakeResult, TLSConfig},
    thread::ThreadHandle,
    types::ShortUInt,
    uri::AMQPUri,
    Error, Promise, PromiseChain, Result,
};
use amq_protocol::frame::{AMQPFrame, ProtocolVersion};
use log::{log_enabled, Level::Trace};
use std::{fmt, io, sync::Arc};

pub struct Connection {
    configuration: Configuration,
    status: ConnectionStatus,
    channels: Channels,
    io_loop: ThreadHandle,
    closer: Arc<ConnectionCloser>,
}

impl Connection {
    fn new(
        waker: SocketStateHandle,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn Executor>,
    ) -> Self {
        let configuration = Configuration::default();
        let status = ConnectionStatus::default();
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            waker,
            internal_rpc.clone(),
            frames,
            executor,
        );
        let closer = Arc::new(ConnectionCloser::new(status.clone(), internal_rpc));
        let connection = Self {
            configuration,
            status,
            channels,
            io_loop: ThreadHandle::default(),
            closer,
        };

        connection.channels.create_zero();
        connection
    }

    /// Connect to an AMQP Server
    pub fn connect(uri: &str, options: ConnectionProperties) -> PromiseChain<Connection> {
        Connect::connect(uri, options, TLSConfig::default())
    }

    /// Connect to an AMQP Server
    pub fn connect_with_config(
        uri: &str,
        options: ConnectionProperties,
        config: TLSConfig<'_, '_, '_>,
    ) -> PromiseChain<Connection> {
        Connect::connect(uri, options, config)
    }

    /// Connect to an AMQP Server
    pub fn connect_uri(uri: AMQPUri, options: ConnectionProperties) -> PromiseChain<Connection> {
        Connect::connect(uri, options, TLSConfig::default())
    }

    /// Connect to an AMQP Server
    pub fn connect_uri_with_identity(
        uri: AMQPUri,
        options: ConnectionProperties,
        config: TLSConfig<'_, '_, '_>,
    ) -> PromiseChain<Connection> {
        Connect::connect(uri, options, config)
    }

    pub fn create_channel(&self) -> PromiseChain<Channel> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidConnectionState(
                self.status.state(),
            )));
        }
        match self.channels.create(self.closer.clone()) {
            Ok(channel) => channel.clone().channel_open(channel),
            Err(error) => PromiseChain::new_with_data(Err(error)),
        }
    }

    /// Block current thread while the connection is still active.
    /// This is useful when you only have a consumer and nothing else keeping your application
    /// "alive".
    pub fn run(self) -> Result<()> {
        let io_loop = self.io_loop.clone();
        drop(self);
        if within_executor() {
            // io_loop waits for the executor to shutdown, do not deadlock in the
            // unlikely case where run is called from within it
            Ok(())
        } else {
            io_loop.wait("io loop")
        }
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

    pub fn close(&self, reply_code: ShortUInt, reply_text: &str) -> Promise<()> {
        self.channels
            .get(0)
            .map(|channel0| channel0.connection_close(reply_code, reply_text, 0, 0))
            .unwrap_or_else(|| Promise::new_with_data(Ok(())))
    }

    /// Block all consumers and publishers on this connection
    pub fn block(&self, reason: &str) -> Promise<()> {
        self.channels
            .get(0)
            .map(|channel0| channel0.connection_blocked(reason))
            .unwrap_or_else(|| {
                Promise::new_with_data(Err(Error::InvalidConnectionState(self.status.state())))
            })
    }

    /// Unblock all consumers and publishers on this connection
    pub fn unblock(&self) -> Promise<()> {
        self.channels
            .get(0)
            .map(|channel0| channel0.connection_unblocked())
            .unwrap_or_else(|| {
                Promise::new_with_data(Err(Error::InvalidConnectionState(self.status.state())))
            })
    }

    /// Update the secret used by some authentication module such as OAuth2
    pub fn update_secret(&self, new_secret: &str, reason: &str) -> Promise<()> {
        self.channels
            .get(0)
            .map(|channel0| channel0.connection_update_secret(new_secret, reason))
            .unwrap_or_else(|| {
                Promise::new_with_data(Err(Error::InvalidConnectionState(self.status.state())))
            })
    }

    pub fn connector(
        mut options: ConnectionProperties,
    ) -> impl FnOnce(AMQPUri, HandshakeResult) -> PromiseChain<Connection> + 'static {
        move |uri, handshake_result| {
            let executor = options
                .executor
                .take()
                .unwrap_or_else(|| Arc::new(DefaultExecutor::default()));
            let reactor_builder = options
                .reactor_builder
                .take()
                .unwrap_or_else(|| Arc::new(DefaultReactorBuilder));
            let socket_state = SocketState::default();
            let waker = socket_state.handle();
            let internal_rpc = InternalRPC::new(executor.clone(), waker.clone());
            let frames = Frames::default();
            let conn = Connection::new(waker, internal_rpc.handle(), frames.clone(), executor);
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
            let (promise, resolver) = Promise::new();
            if log_enabled!(Trace) {
                promise.set_marker("ProtocolHeader".into());
            }
            let channels = conn.channels.clone();
            if let Some(channel0) = channels.get(0) {
                channel0.send_frame(
                    AMQPFrame::ProtocolHeader(ProtocolVersion::amqp_0_9_1()),
                    resolver,
                    None,
                )
            };
            let (promise, resolver) = PromiseChain::after(promise);
            if log_enabled!(Trace) {
                promise.set_marker("ProtocolHeader.Ok".into());
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
            IoLoop::new(
                status,
                configuration,
                channels,
                internal_rpc,
                frames,
                socket_state,
                io_loop_handle,
                handshake_result,
                &*reactor_builder,
            )
            .and_then(IoLoop::start)
            .map(|()| promise)
            .unwrap_or_else(|err| PromiseChain::new_with_data(Err(err)))
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
pub trait Connect {
    /// connect to an AMQP server
    fn connect(
        self,
        options: ConnectionProperties,
        config: TLSConfig<'_, '_, '_>,
    ) -> PromiseChain<Connection>;
}

impl Connect for AMQPUri {
    fn connect(
        self,
        options: ConnectionProperties,
        config: TLSConfig<'_, '_, '_>,
    ) -> PromiseChain<Connection> {
        let stream = AMQPUriTcpExt::connect_with_config(&self, config);
        Connection::connector(options)(self, stream)
    }
}

impl Connect for &str {
    fn connect(
        self,
        options: ConnectionProperties,
        config: TLSConfig<'_, '_, '_>,
    ) -> PromiseChain<Connection> {
        match self.parse::<AMQPUri>() {
            Ok(uri) => Connect::connect(uri, options, config),
            Err(err) => {
                PromiseChain::new_with_data(Err(io::Error::new(io::ErrorKind::Other, err).into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use env_logger;

    use super::*;
    use crate::channel_receiver_state::ChannelReceiverState;
    use crate::channel_status::ChannelState;
    use crate::types::ShortString;
    use crate::BasicProperties;
    use amq_protocol::frame::AMQPContentHeader;
    use amq_protocol::protocol::{basic, AMQPClass};

    #[test]
    fn basic_consume_small_payload() {
        let _ = env_logger::try_init();

        use crate::consumer::Consumer;
        use crate::queue::{Queue, QueueState};

        // Bootstrap connection state to a consuming state
        let executor = Arc::new(DefaultExecutor::default());
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
        let mut queue: QueueState = Queue::new(queue_name.clone(), 0, 0).into();
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(consumer_tag.clone(), executor);
        queue.register_consumer(consumer_tag.clone(), consumer);
        conn.channels
            .get(channel.id())
            .map(|c| c.register_queue(queue));
        // Now test the state machine behaviour
        {
            let method = AMQPClass::Basic(basic::AMQPMethod::Deliver(basic::Deliver {
                consumer_tag: consumer_tag.clone(),
                delivery_tag: 1,
                redelivered: false,
                exchange: "".into(),
                routing_key: queue_name.clone(),
            }));
            let class_id = method.get_amqp_class_id();
            let deliver_frame = AMQPFrame::Method(channel.id(), method);
            conn.channels.handle_frame(deliver_frame).unwrap();
            let channel_state = channel.status().receiver_state();
            let expected_state = ChannelReceiverState::WillReceiveContent(
                class_id,
                Some(queue_name.clone()),
                Some(consumer_tag.clone()),
            );
            assert_eq!(channel_state, expected_state);
        }
        {
            let header_frame = AMQPFrame::Header(
                channel.id(),
                60,
                Box::new(AMQPContentHeader {
                    class_id: 60,
                    weight: 0,
                    body_size: 2,
                    properties: BasicProperties::default(),
                }),
            );
            conn.channels.handle_frame(header_frame).unwrap();
            let channel_state = channel.status().receiver_state();
            let expected_state = ChannelReceiverState::ReceivingContent(
                Some(queue_name.clone()),
                Some(consumer_tag.clone()),
                2,
            );
            assert_eq!(channel_state, expected_state);
        }
        {
            let body_frame = AMQPFrame::Body(channel.id(), "{}".as_bytes().to_vec());
            conn.channels.handle_frame(body_frame).unwrap();
            let channel_state = channel.status().state();
            let expected_state = ChannelState::Connected;
            assert_eq!(channel_state, expected_state);
        }
    }

    #[test]
    fn basic_consume_empty_payload() {
        let _ = env_logger::try_init();

        use crate::consumer::Consumer;
        use crate::queue::{Queue, QueueState};

        // Bootstrap connection state to a consuming state
        let socket_state = SocketState::default();
        let waker = socket_state.handle();
        let executor = Arc::new(DefaultExecutor::default());
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
        let mut queue: QueueState = Queue::new(queue_name.clone(), 0, 0).into();
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(consumer_tag.clone(), executor);
        queue.register_consumer(consumer_tag.clone(), consumer);
        conn.channels
            .get(channel.id())
            .map(|c| c.register_queue(queue));
        // Now test the state machine behaviour
        {
            let method = AMQPClass::Basic(basic::AMQPMethod::Deliver(basic::Deliver {
                consumer_tag: consumer_tag.clone(),
                delivery_tag: 1,
                redelivered: false,
                exchange: "".into(),
                routing_key: queue_name.clone(),
            }));
            let class_id = method.get_amqp_class_id();
            let deliver_frame = AMQPFrame::Method(channel.id(), method);
            conn.channels.handle_frame(deliver_frame).unwrap();
            let channel_state = channel.status().receiver_state();
            let expected_state = ChannelReceiverState::WillReceiveContent(
                class_id,
                Some(queue_name.clone()),
                Some(consumer_tag.clone()),
            );
            assert_eq!(channel_state, expected_state);
        }
        {
            let header_frame = AMQPFrame::Header(
                channel.id(),
                60,
                Box::new(AMQPContentHeader {
                    class_id: 60,
                    weight: 0,
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
