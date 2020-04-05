use crate::{
    channel::{Channel, Reply},
    channels::Channels,
    close_on_drop,
    configuration::Configuration,
    connection_properties::ConnectionProperties,
    connection_status::{ConnectionState, ConnectionStatus},
    error_handler::ErrorHandler,
    executor::DefaultExecutor,
    executor::Executor,
    frames::{ExpectedReply, Frames, Priority},
    io_loop::IoLoop,
    promises::Promises,
    tcp::{AMQPUriTcpExt, Identity, TcpStream},
    thread::{JoinHandle, ThreadHandle},
    types::ShortUInt,
    uri::AMQPUri,
    waker::Waker,
    CloseOnDropPromise, Error, Promise, PromiseResolver, Result,
};
use amq_protocol::frame::AMQPFrame;
use log::{debug, error, log_enabled, trace, Level::Trace};
use mio::{Poll, Token, Waker as MioWaker};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Connection {
    configuration: Configuration,
    status: ConnectionStatus,
    channels: Channels,
    waker: Waker,
    frames: Frames,
    io_loop: ThreadHandle,
    internal_promises: Promises<()>,
    error_handler: ErrorHandler,
}

impl Default for Connection {
    fn default() -> Self {
        Self::new(DefaultExecutor::default())
    }
}

impl Connection {
    fn new(executor: Arc<dyn Executor>) -> Self {
        let frames = Frames::default();
        let connection = Self {
            configuration: Configuration::default(),
            status: ConnectionStatus::default(),
            channels: Channels::new(frames.clone(), executor),
            waker: Waker::default(),
            frames,
            io_loop: ThreadHandle::default(),
            internal_promises: Promises::default(),
            error_handler: ErrorHandler::default(),
        };

        connection.channels.create_zero(connection.clone());
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
        match self.channels.create(self.clone()) {
            Ok(channel) => channel.channel_open(),
            Err(error) => CloseOnDropPromise::new_with_data(Err(error)),
        }
    }

    pub(crate) fn remove_channel(&self, channel_id: u16, error: Error) -> Result<()> {
        self.channels.remove(channel_id, error)
    }

    /// Block current thread while the connection is still active.
    /// This is useful when you only have a consumer and nothing else keeping your application
    /// "alive".
    pub fn run(&self) -> Result<()> {
        self.io_loop.wait("io loop")
    }

    pub fn on_error<E: Fn(Error) + Send + 'static>(&self, handler: Box<E>) {
        self.error_handler.set_handler(handler);
    }

    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    pub(crate) fn flow(&self) -> bool {
        self.channels.flow()
    }

    fn channel0(&self) -> Channel {
        self.channels.get(0).expect("channel 0")
    }

    pub fn close(&self, reply_code: ShortUInt, reply_text: &str) -> Promise<()> {
        self.channel0()
            .connection_close(reply_code, reply_text, 0, 0)
    }

    /// Block all consumers and publishers on this connection
    pub fn block(&self, reason: &str) -> Promise<()> {
        self.channel0().connection_blocked(reason)
    }

    /// Unblock all consumers and publishers on this connection
    pub fn unblock(&self) -> Promise<()> {
        self.channel0().connection_unblocked()
    }

    /// Update the secret used by some authentication module such as oauth2
    pub fn update_secret(&self, new_secret: &str, reason: &str) -> Promise<()> {
        self.channel0().connection_update_secret(new_secret, reason)
    }

    pub(crate) fn set_io_loop(
        &mut self,
        io_loop_handle: JoinHandle,
        waker: Arc<MioWaker>,
    ) -> Result<()> {
        self.io_loop.register(io_loop_handle);
        self.waker.set_waker(waker)
    }

    pub(crate) fn has_pending_frames(&self) -> bool {
        self.frames.has_pending()
    }

    pub(crate) fn drop_pending_frames(&self, error: Error) {
        self.frames.drop_pending(error);
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
            let executor = options
                .executor
                .take()
                .unwrap_or_else(|| DefaultExecutor::new(options.max_executor_threads));
            let conn = Connection::new(executor);
            conn.status.set_vhost(&uri.vhost);
            conn.status.set_username(&uri.authority.userinfo.username);
            if let Some(frame_max) = uri.query.frame_max {
                conn.configuration.set_frame_max(frame_max);
            }
            if let Some(channel_max) = uri.query.channel_max {
                conn.configuration.set_channel_max(channel_max);
            }
            if let Some(heartbeat) = uri.query.heartbeat {
                conn.configuration.set_heartbeat(heartbeat);
            }
            let (promise, resolver) = Promise::new();
            if log_enabled!(Trace) {
                promise.set_marker("ProtocolHeader".into());
            }
            if let Err(err) = conn.send_frame(
                0,
                Priority::CRITICAL,
                AMQPFrame::ProtocolHeader,
                resolver.clone(),
                None,
            ) {
                resolver.swear(Err(err));
            }
            let (promise, resolver) = CloseOnDropPromise::after(promise);
            if log_enabled!(Trace) {
                promise.set_marker("ProtocolHeader.Ok".into());
            }
            conn.set_state(ConnectionState::SentProtocolHeader(
                resolver,
                uri.authority.userinfo.into(),
                options,
            ));
            IoLoop::new(conn, stream, poll)?.start()?;
            Ok(promise)
        }
    }

    pub(crate) fn set_state(&self, state: ConnectionState) {
        self.status.set_state(state);
    }

    pub(crate) fn do_block(&self) {
        self.status.block();
    }

    pub(crate) fn do_unblock(&self) -> Result<()> {
        self.status.unblock();
        self.wake()
    }

    pub(crate) fn wake(&self) -> Result<()> {
        trace!("connection wake");
        self.waker.wake()
    }

    pub(crate) fn send_frame(
        &self,
        channel_id: u16,
        priority: Priority,
        frame: AMQPFrame,
        resolver: PromiseResolver<()>,
        expected_reply: Option<ExpectedReply>,
    ) -> Result<()> {
        trace!("connection send_frame; channel_id={}", channel_id);
        self.frames
            .push(channel_id, priority, frame, resolver, expected_reply);
        self.wake()?;
        Ok(())
    }

    pub(crate) fn send_frames(
        &self,
        channel_id: u16,
        frames: Vec<AMQPFrame>,
    ) -> Result<Promise<()>> {
        trace!("connection send_frames; channel_id={}", channel_id);
        let promise = self.frames.push_frames(frames);
        self.wake()?;
        Ok(promise)
    }

    pub(crate) fn next_expected_reply(&self, channel_id: u16) -> Option<Reply> {
        self.frames.next_expected_reply(channel_id)
    }

    /// next message to send to the network
    ///
    /// returns None if there's no message to send
    pub(crate) fn next_frame(&self) -> Option<(AMQPFrame, Option<PromiseResolver<()>>)> {
        self.frames.pop(self.flow())
    }

    /// updates the current state with a new received frame
    pub(crate) fn handle_frame(&self, f: AMQPFrame) -> Result<()> {
        if let Err(err) = self.do_handle_frame(f) {
            self.set_error(err.clone())?;
            Err(err)
        } else {
            Ok(())
        }
    }

    fn do_handle_frame(&self, f: AMQPFrame) -> Result<()> {
        trace!("will handle frame: {:?}", f);
        match f {
            AMQPFrame::ProtocolHeader => {
                error!("error: the client should not receive a protocol header");
                return Err(Error::InvalidFrameReceived);
            }
            AMQPFrame::Method(channel_id, method) => {
                self.channels.receive_method(channel_id, method)?;
            }
            AMQPFrame::Heartbeat(_) => {
                debug!("received heartbeat from server");
            }
            AMQPFrame::Header(channel_id, _, header) => {
                self.channels.handle_content_header_frame(
                    channel_id,
                    header.body_size,
                    header.properties,
                )?;
            }
            AMQPFrame::Body(channel_id, payload) => {
                self.channels.handle_body_frame(channel_id, payload)?;
            }
        }
        Ok(())
    }

    pub(crate) fn send_heartbeat(&self) -> Result<()> {
        self.wake()?;
        let (promise, resolver) = Promise::new();

        if log_enabled!(Trace) {
            promise.set_marker("Heartbeat".into());
        }

        self.send_frame(
            0,
            Priority::CRITICAL,
            AMQPFrame::Heartbeat(0),
            resolver,
            None,
        )?;
        self.register_internal_promise(promise)
    }

    pub(crate) fn requeue_frame(
        &self,
        frame: (AMQPFrame, Option<PromiseResolver<()>>),
    ) -> Result<()> {
        self.wake()?;
        self.frames.retry(frame);
        Ok(())
    }

    pub(crate) fn set_closing(&self) {
        self.set_state(ConnectionState::Closing);
        self.channels.set_closing();
    }

    pub(crate) fn set_closed(&self, error: Error) -> Result<()> {
        self.set_state(ConnectionState::Closed);
        self.channels.set_closed(error)
    }

    pub(crate) fn set_error(&self, error: Error) -> Result<()> {
        error!("Connection error");
        self.set_state(ConnectionState::Error);
        self.error_handler.on_error(error.clone());
        self.channels.set_error(error)
    }

    pub(crate) fn register_internal_promise(&self, promise: Promise<()>) -> Result<()> {
        self.internal_promises.register(promise).unwrap_or(Ok(()))
    }

    pub(crate) fn poll_internal_promises(&self) -> Result<()> {
        if let Some(results) = self.internal_promises.try_wait() {
            for res in results {
                if let Err(err) = res {
                    self.set_error(err)?;
                }
            }
        }
        Ok(())
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
        let conn = Connection::default();
        conn.set_state(ConnectionState::Connected);
        conn.configuration.set_channel_max(2047);
        let channel = conn.channels.create(conn.clone()).unwrap();
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
            conn.handle_frame(deliver_frame).unwrap();
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
            conn.handle_frame(header_frame).unwrap();
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
            conn.handle_frame(body_frame).unwrap();
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
        let conn = Connection::default();
        conn.set_state(ConnectionState::Connected);
        conn.configuration.set_channel_max(2047);
        let channel = conn.channels.create(conn.clone()).unwrap();
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
            conn.handle_frame(deliver_frame).unwrap();
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
            conn.handle_frame(header_frame).unwrap();
            let channel_state = channel.status().state();
            let expected_state = ChannelState::Connected;
            assert_eq!(channel_state, expected_state);
        }
    }
}

impl close_on_drop::__private::Closable for Connection {
    fn close(&self, reply_code: ShortUInt, reply_text: &str) -> Promise<()> {
        Connection::close(self, reply_code, reply_text)
    }
}
