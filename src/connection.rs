use crate::{
    channel::{Channel, Reply},
    channels::Channels,
    configuration::Configuration,
    confirmation::Confirmation,
    connection_properties::ConnectionProperties,
    connection_status::{ConnectionState, ConnectionStatus},
    error_handler::ErrorHandler,
    executor::DefaultExecutor,
    executor::Executor,
    frames::{Frames, Priority, SendId},
    io_loop::{IoLoop, IoLoopHandle},
    registration::Registration,
    tcp::{AMQPUriTcpExt, Identity, TcpStream},
    types::ShortUInt,
    wait::{Cancellable, Wait},
    Error,
};
use amq_protocol::{frame::AMQPFrame, uri::AMQPUri};
use log::{debug, error, trace};
use mio::{Evented, Poll, PollOpt, Ready, Token};
use std::{io, sync::Arc, thread::JoinHandle};

#[derive(Clone, Debug)]
pub struct Connection {
    configuration: Configuration,
    status: ConnectionStatus,
    channels: Channels,
    registration: Registration,
    frames: Frames,
    io_loop: IoLoopHandle,
    error_handler: ErrorHandler,
}

impl Connection {
    fn new(executor: Arc<dyn Executor>) -> Self {
        let frames = Frames::default();
        let connection = Self {
            configuration: Configuration::default(),
            status: ConnectionStatus::default(),
            channels: Channels::new(frames.clone(), executor),
            registration: Registration::default(),
            frames,
            io_loop: IoLoopHandle::default(),
            error_handler: ErrorHandler::default(),
        };

        connection.channels.create_zero(connection.clone());
        connection
    }
}

impl Default for Connection {
    fn default() -> Self {
        Self::new(DefaultExecutor::default())
    }
}

impl Evented for Connection {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.registration.register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.registration.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.registration.deregister(poll)
    }
}

impl Connection {
    /// Connect to an AMQP Server
    pub fn connect(uri: &str, options: ConnectionProperties) -> Confirmation<Connection> {
        Connect::connect(uri, options, None)
    }

    /// Connect to an AMQP Server
    pub fn connect_with_identity(
        uri: &str,
        options: ConnectionProperties,
        identity: Identity<'_, '_>,
    ) -> Confirmation<Connection> {
        Connect::connect(uri, options, Some(identity))
    }

    /// Connect to an AMQP Server
    pub fn connect_uri(uri: AMQPUri, options: ConnectionProperties) -> Confirmation<Connection> {
        Connect::connect(uri, options, None)
    }

    /// Connect to an AMQP Server
    pub fn connect_uri_with_identity(
        uri: AMQPUri,
        options: ConnectionProperties,
        identity: Identity<'_, '_>,
    ) -> Confirmation<Connection> {
        Connect::connect(uri, options, Some(identity))
    }

    pub fn create_channel(&self) -> Confirmation<Channel> {
        if !self.status.connected() {
            return Confirmation::new_error(Error::InvalidConnectionState(self.status.state()));
        }
        match self.channels.create(self.clone()) {
            Ok(channel) => channel.channel_open(),
            Err(error) => Confirmation::new_error(error),
        }
    }

    pub(crate) fn remove_channel(&self, channel_id: u16) -> Result<(), Error> {
        self.channels.remove(channel_id)
    }

    /// Block current thread while the connection is still active.
    /// This is useful when you only have a consumer and nothing else keeping your application
    /// "alive".
    pub fn run(&self) -> Result<(), Error> {
        self.io_loop.wait()
    }

    pub fn on_error<E: Fn() + Send + 'static>(&self, handler: Box<E>) {
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

    pub fn close(&self, reply_code: ShortUInt, reply_text: &str) -> Confirmation<()> {
        self.channel0()
            .connection_close(reply_code, reply_text, 0, 0)
    }

    /// Block all consumers and publishers on this connection
    pub fn block(&self, reason: &str) -> Confirmation<()> {
        self.channel0().connection_blocked(reason)
    }

    /// Unblock all consumers and publishers on this connection
    pub fn unblock(&self) -> Confirmation<()> {
        self.channel0().connection_unblocked()
    }

    /// Update the secret used by some authentication module such as oauth2
    pub fn update_secret(&self, new_secret: &str, reason: &str) -> Confirmation<()> {
        self.channel0().connection_update_secret(new_secret, reason)
    }

    pub(crate) fn set_io_loop(&self, io_loop: JoinHandle<Result<(), Error>>) {
        self.io_loop.register(io_loop);
    }

    pub(crate) fn drop_pending_frames(&self) {
        self.frames.drop_pending();
    }

    pub fn connector(
        mut options: ConnectionProperties,
    ) -> impl FnOnce(TcpStream, AMQPUri, Option<(Poll, Token)>) -> Result<Wait<Connection>, Error>
                 + 'static {
        move |stream, uri, poll| {
            let executor = options
                .executor
                .take()
                .unwrap_or_else(|| DefaultExecutor::new(options.max_executor_threads));
            let conn = Connection::new(executor);
            conn.status.set_vhost(&uri.vhost);
            if let Some(frame_max) = uri.query.frame_max {
                conn.configuration.set_frame_max(frame_max);
            }
            if let Some(channel_max) = uri.query.channel_max {
                conn.configuration.set_channel_max(channel_max);
            }
            if let Some(heartbeat) = uri.query.heartbeat {
                conn.configuration.set_heartbeat(heartbeat);
            }
            conn.send_frame(0, Priority::CRITICAL, AMQPFrame::ProtocolHeader, None)?;
            let (wait, wait_handle) = Wait::new();
            conn.set_state(ConnectionState::SentProtocolHeader(
                wait_handle,
                uri.authority.userinfo.into(),
                options,
            ));
            IoLoop::new(conn.clone(), stream, poll)?.run()?;
            Ok(wait)
        }
    }

    pub(crate) fn set_state(&self, state: ConnectionState) {
        self.status.set_state(state);
    }

    pub(crate) fn do_block(&self) {
        self.status.block();
    }

    pub(crate) fn do_unblock(&self) -> Result<(), Error> {
        self.status.unblock();
        self.set_readable()
    }

    fn set_readable(&self) -> Result<(), Error> {
        trace!("connection set readable");
        self.registration
            .set_readiness(Ready::readable())
            .map_err(Error::IOError)?;
        Ok(())
    }

    pub(crate) fn send_frame(
        &self,
        channel_id: u16,
        priority: Priority,
        frame: AMQPFrame,
        expected_reply: Option<(Reply, Box<dyn Cancellable + Send>)>,
    ) -> Result<Wait<()>, Error> {
        trace!("connection send_frame; channel_id={}", channel_id);
        let wait = self
            .frames
            .push(channel_id, priority, frame, expected_reply);
        self.set_readable()?;
        Ok(wait)
    }

    pub(crate) fn send_frames(
        &self,
        channel_id: u16,
        frames: Vec<(AMQPFrame, Option<AMQPFrame>)>,
    ) -> Result<Wait<()>, Error> {
        trace!("connection send_frames; channel_id={}", channel_id);
        let wait = self.frames.push_frames(channel_id, frames);
        self.set_readable()?;
        Ok(wait)
    }

    pub(crate) fn next_expected_reply(&self, channel_id: u16) -> Option<Reply> {
        self.frames.next_expected_reply(channel_id)
    }

    /// next message to send to the network
    ///
    /// returns None if there's no message to send
    pub(crate) fn next_frame(&self) -> Option<(SendId, AMQPFrame)> {
        self.frames.pop(self.flow())
    }

    /// updates the current state with a new received frame
    pub(crate) fn handle_frame(&self, f: AMQPFrame) -> Result<(), Error> {
        if let Err(err) = self.do_handle_frame(f) {
            self.set_error()?;
            Err(err)
        } else {
            Ok(())
        }
    }

    fn do_handle_frame(&self, f: AMQPFrame) -> Result<(), Error> {
        trace!("will handle frame: {:?}", f);
        match f {
            AMQPFrame::ProtocolHeader => {
                error!("error: the client should not receive a protocol header");
                return Err(Error::InvalidConnectionState(ConnectionState::Connected));
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

    pub(crate) fn send_heartbeat(&self) -> Result<(), Error> {
        self.set_readable()?;
        self.send_frame(0, Priority::CRITICAL, AMQPFrame::Heartbeat(0), None)?;
        Ok(())
    }

    pub(crate) fn requeue_frame(&self, send_id: SendId, frame: AMQPFrame) -> Result<(), Error> {
        self.set_readable()?;
        self.frames.retry(send_id, frame);
        Ok(())
    }

    pub(crate) fn mark_sent(&self, send_id: SendId) {
        self.frames.mark_sent(send_id);
    }

    pub(crate) fn set_closing(&self) {
        self.set_state(ConnectionState::Closing);
        self.channels.set_closing();
    }

    pub(crate) fn set_closed(&self) -> Result<(), Error> {
        self.set_state(ConnectionState::Closed);
        self.channels.set_closed()
    }

    pub(crate) fn set_error(&self) -> Result<(), Error> {
        error!("Connection error");
        self.set_state(ConnectionState::Error);
        self.channels.set_error()?;
        self.error_handler.on_error();
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
    ) -> Confirmation<Connection>
    where
        Self: Sized,
    {
        match Poll::new().map_err(Error::IOError) {
            Ok(poll) => self
                .connect_raw(options, Some((poll, crate::io_loop::SOCKET)), identity)
                .into(),
            Err(err) => Confirmation::new_error(err),
        }
    }

    /// connect to an AMQP server, for internal use
    fn connect_raw(
        self,
        options: ConnectionProperties,
        poll: Option<(Poll, Token)>,
        identity: Option<Identity<'_, '_>>,
    ) -> Result<Wait<Connection>, Error>;
}

impl Connect for AMQPUri {
    fn connect_raw(
        self,
        options: ConnectionProperties,
        poll: Option<(Poll, Token)>,
        identity: Option<Identity<'_, '_>>,
    ) -> Result<Wait<Connection>, Error> {
        AMQPUriTcpExt::connect_full(self, Connection::connector(options), poll, identity)
            .map_err(Error::IOError)?
    }
}

impl Connect for &str {
    fn connect_raw(
        self,
        options: ConnectionProperties,
        poll: Option<(Poll, Token)>,
        identity: Option<Identity<'_, '_>>,
    ) -> Result<Wait<Connection>, Error> {
        AMQPUriTcpExt::connect_full(self, Connection::connector(options), poll, identity)
            .map_err(Error::IOError)?
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
