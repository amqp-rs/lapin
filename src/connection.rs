use crate::{
    channel::Channel,
    channels::Channels,
    close_on_drop,
    configuration::Configuration,
    connection_properties::ConnectionProperties,
    connection_status::{ConnectionState, ConnectionStatus},
    executor::DefaultExecutor,
    executor::Executor,
    frames::Frames,
    internal_rpc::{InternalRPC, InternalRPCHandle},
    io_loop::IoLoop,
    promises::Promises,
    tcp::{AMQPUriTcpExt, Identity, TcpStream},
    thread::ThreadHandle,
    types::ShortUInt,
    uri::AMQPUri,
    waker::Waker,
    CloseOnDropPromise, Error, Promise, Result,
};
use amq_protocol::frame::AMQPFrame;
use log::{log_enabled, Level::Trace};
use mio::{Poll, Token};
use std::{fmt, sync::Arc};

#[derive(Clone)]
pub struct Connection {
    configuration: Configuration,
    status: ConnectionStatus,
    channels: Channels,
    io_loop: ThreadHandle,
}

impl Connection {
    fn new(
        waker: Waker,
        internal_rpc: InternalRPCHandle,
        internal_promises: Promises<()>,
        frames: Frames,
        executor: Arc<dyn Executor>,
    ) -> Self {
        let configuration = Configuration::default();
        let status = ConnectionStatus::default();
        let channels = Channels::new(
            configuration.clone(),
            status.clone(),
            waker,
            internal_rpc,
            frames,
            internal_promises,
            executor,
        );
        let connection = Self {
            configuration,
            status,
            channels,
            io_loop: ThreadHandle::default(),
        };

        connection.channels.create_zero();
        connection
    }

    /// Connect to an AMQP Server
    pub fn connect(uri: &str, options: ConnectionProperties) -> CloseOnDropPromise<Connection> {
        Connect::connect(uri, options, None)
    }

    /// Connect to an AMQP Server
    pub fn connect_with_identity(
        uri: &str,
        options: ConnectionProperties,
        identity: Identity<'_, '_>,
    ) -> CloseOnDropPromise<Connection> {
        Connect::connect(uri, options, Some(identity))
    }

    /// Connect to an AMQP Server
    pub fn connect_uri(
        uri: AMQPUri,
        options: ConnectionProperties,
    ) -> CloseOnDropPromise<Connection> {
        Connect::connect(uri, options, None)
    }

    /// Connect to an AMQP Server
    pub fn connect_uri_with_identity(
        uri: AMQPUri,
        options: ConnectionProperties,
        identity: Identity<'_, '_>,
    ) -> CloseOnDropPromise<Connection> {
        Connect::connect(uri, options, Some(identity))
    }

    pub fn create_channel(&self) -> CloseOnDropPromise<Channel> {
        if !self.status.connected() {
            return CloseOnDropPromise::new_with_data(Err(Error::InvalidConnectionState(
                self.status.state(),
            )));
        }
        match self.channels.create() {
            Ok(channel) => channel.channel_open(),
            Err(error) => CloseOnDropPromise::new_with_data(Err(error)),
        }
    }

    /// Block current thread while the connection is still active.
    /// This is useful when you only have a consumer and nothing else keeping your application
    /// "alive".
    pub fn run(&self) -> Result<()> {
        self.io_loop.wait("io loop")
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

    fn channel0(&self) -> Channel {
        self.channels.get(0).expect("channel 0")
    }

    pub fn close(&self, reply_code: ShortUInt, reply_text: &str) -> Promise<()> {
        if let Some(channel0) = self.channels.get(0) {
            channel0.connection_close(reply_code, reply_text, 0, 0)
        } else {
            Promise::new_with_data(Ok(()))
        }
    }

    /// Block all consumers and publishers on this connection
    pub fn block(&self, reason: &str) -> Promise<()> {
        if let Some(channel0) = self.channels.get(0) {
            channel0.connection_blocked(reason)
        } else {
            Promise::new_with_data(Err(Error::InvalidConnectionState(self.status.state())))
        }
    }

    /// Unblock all consumers and publishers on this connection
    pub fn unblock(&self) -> Promise<()> {
        if let Some(channel0) = self.channels.get(0) {
            channel0.connection_unblocked()
        } else {
            Promise::new_with_data(Err(Error::InvalidConnectionState(self.status.state())))
        }
    }

    /// Update the secret used by some authentication module such as oauth2
    pub fn update_secret(&self, new_secret: &str, reason: &str) -> Promise<()> {
        if let Some(channel0) = self.channels.get(0) {
            channel0.connection_update_secret(new_secret, reason)
        } else {
            Promise::new_with_data(Err(Error::InvalidConnectionState(self.status.state())))
        }
    }

    pub fn connector(
        mut options: ConnectionProperties,
    ) -> impl FnOnce(
        TcpStream,
        AMQPUri,
        Option<(Poll, Token)>,
    ) -> Result<CloseOnDropPromise<Connection>>
           + 'static {
        move |stream, uri, poll| {
            let waker = Waker::default();
            let internal_promises = Promises::default();
            let internal_rpc = InternalRPC::new(waker.clone(), internal_promises.clone());
            let frames = Frames::default();
            let executor = options
                .executor
                .take()
                .unwrap_or_else(|| DefaultExecutor::new(options.max_executor_threads));
            let conn = Connection::new(
                waker.clone(),
                internal_rpc.handle(),
                internal_promises,
                frames.clone(),
                executor,
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
            let (promise, resolver) = Promise::new();
            if log_enabled!(Trace) {
                promise.set_marker("ProtocolHeader".into());
            }
            if let Err(err) =
                conn.channel0()
                    .send_frame(AMQPFrame::ProtocolHeader, resolver.clone(), None)
            {
                resolver.swear(Err(err));
            }
            let (promise, resolver) = CloseOnDropPromise::after(promise);
            if log_enabled!(Trace) {
                promise.set_marker("ProtocolHeader.Ok".into());
            }
            let channels = conn.channels.clone();
            let io_loop_handle = conn.io_loop.clone();
            status.set_state(ConnectionState::SentProtocolHeader(
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
                io_loop_handle,
                waker,
                stream,
                poll,
            )?
            .start()?;
            Ok(promise)
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
        identity: Option<Identity<'_, '_>>,
    ) -> CloseOnDropPromise<Connection>
    where
        Self: Sized,
    {
        Poll::new()
            .map_err(Error::from)
            .and_then(|poll| self.connect_raw(options, Some(poll), identity))
            .unwrap_or_else(|err| CloseOnDropPromise::new_with_data(Err(err)))
    }

    /// connect to an AMQP server, for internal use
    fn connect_raw(
        self,
        options: ConnectionProperties,
        poll: Option<Poll>,
        identity: Option<Identity<'_, '_>>,
    ) -> Result<CloseOnDropPromise<Connection>>;
}

impl Connect for AMQPUri {
    fn connect_raw(
        self,
        options: ConnectionProperties,
        poll: Option<Poll>,
        identity: Option<Identity<'_, '_>>,
    ) -> Result<CloseOnDropPromise<Connection>> {
        AMQPUriTcpExt::connect_full(
            self,
            Connection::connector(options),
            poll.map(|poll| (poll, crate::io_loop::SOCKET)),
            identity,
        )?
    }
}

impl Connect for &str {
    fn connect_raw(
        self,
        options: ConnectionProperties,
        poll: Option<Poll>,
        identity: Option<Identity<'_, '_>>,
    ) -> Result<CloseOnDropPromise<Connection>> {
        AMQPUriTcpExt::connect_full(
            self,
            Connection::connector(options),
            poll.map(|poll| (poll, crate::io_loop::SOCKET)),
            identity,
        )?
    }
}

#[cfg(test)]
mod tests {
    use env_logger;

    use super::*;
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
        let waker = Waker::default();
        let internal_promises = Promises::default();
        let conn = Connection::new(
            waker.clone(),
            InternalRPC::new(waker, internal_promises.clone()).handle(),
            internal_promises,
            Frames::default(),
            DefaultExecutor::default(),
        );
        conn.status.set_state(ConnectionState::Connected);
        conn.configuration.set_channel_max(2047);
        let channel = conn.channels.create().unwrap();
        channel.set_state(ChannelState::Connected);
        let queue_name = ShortString::from("consumed");
        let mut queue: QueueState = Queue::new(queue_name.clone(), 0, 0).into();
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(consumer_tag.clone(), DefaultExecutor::default());
        queue.register_consumer(consumer_tag.clone(), consumer);
        if let Some(c) = conn.channels.get(channel.id()) {
            c.register_queue(queue);
        }
        // Now test the state machine behaviour
        {
            let deliver_frame = AMQPFrame::Method(
                channel.id(),
                AMQPClass::Basic(basic::AMQPMethod::Deliver(basic::Deliver {
                    consumer_tag: consumer_tag.clone(),
                    delivery_tag: 1,
                    redelivered: false,
                    exchange: "".into(),
                    routing_key: queue_name.clone(),
                })),
            );
            conn.channels.handle_frame(deliver_frame).unwrap();
            let channel_state = channel.status().state();
            let expected_state = ChannelState::WillReceiveContent(
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
            let channel_state = channel.status().state();
            let expected_state = ChannelState::ReceivingContent(
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
        let waker = Waker::default();
        let internal_promises = Promises::default();
        let conn = Connection::new(
            waker.clone(),
            InternalRPC::new(waker, internal_promises.clone()).handle(),
            internal_promises,
            Frames::default(),
            DefaultExecutor::default(),
        );
        conn.status.set_state(ConnectionState::Connected);
        conn.configuration.set_channel_max(2047);
        let channel = conn.channels.create().unwrap();
        channel.set_state(ChannelState::Connected);
        let queue_name = ShortString::from("consumed");
        let mut queue: QueueState = Queue::new(queue_name.clone(), 0, 0).into();
        let consumer_tag = ShortString::from("consumer-tag");
        let consumer = Consumer::new(consumer_tag.clone(), DefaultExecutor::default());
        queue.register_consumer(consumer_tag.clone(), consumer);
        conn.channels.get(channel.id()).map(|c| {
            c.register_queue(queue);
        });
        // Now test the state machine behaviour
        {
            let deliver_frame = AMQPFrame::Method(
                channel.id(),
                AMQPClass::Basic(basic::AMQPMethod::Deliver(basic::Deliver {
                    consumer_tag: consumer_tag.clone(),
                    delivery_tag: 1,
                    redelivered: false,
                    exchange: "".into(),
                    routing_key: queue_name.clone(),
                })),
            );
            conn.channels.handle_frame(deliver_frame).unwrap();
            let channel_state = channel.status().state();
            let expected_state = ChannelState::WillReceiveContent(
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

impl close_on_drop::__private::Closable for Connection {
    fn close(&self, reply_code: ShortUInt, reply_text: &str) -> Promise<()> {
        if self.status().connected() {
            Connection::close(self, reply_code, reply_text)
        } else {
            Promise::new_with_data(Ok(()))
        }
    }
}
