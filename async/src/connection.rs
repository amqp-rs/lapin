use amq_protocol::{
  frame::AMQPFrame,
  tcp::TcpStream,
  uri::AMQPUri,
};
use mio::{Evented, Poll, PollOpt, Ready, Token};
use log::{debug, error, trace};

use std::io;

use crate::{
  channel::{Channel, Reply},
  channels::Channels,
  confirmation::Confirmation,
  configuration::Configuration,
  connection_properties::ConnectionProperties,
  connection_status::{ConnectionStatus, ConnectionState},
  credentials::Credentials,
  error::{Error, ErrorKind},
  frames::{Frames, Priority, SendId},
  io_loop::IoLoop,
  registration::Registration,
  tcp::AMQPUriTcpExt,
  wait::Wait,
};

#[derive(Clone, Debug)]
pub struct Connection {
  pub        configuration: Configuration,
  pub        status:        ConnectionStatus,
  pub(crate) channels:      Channels,
             registration:  Registration,
             frames:        Frames,
}

impl Default for Connection {
  fn default() -> Self {
    let connection = Self {
      configuration: Configuration::default(),
      status:        ConnectionStatus::default(),
      channels:      Channels::default(),
      registration:  Registration::default(),
      frames:        Frames::default(),
    };

    connection.channels.create_zero(connection.clone());
    connection
  }
}

impl Evented for Connection {
  fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
    self.registration.register(poll, token, interest, opts)
  }

  fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
    self.registration.reregister(poll, token, interest, opts)
  }

  fn deregister(&self, poll: &Poll) -> io::Result<()> {
    self.registration.deregister(poll)
  }
}

impl Connection {
  /// Connect to an AMQP Server
  pub fn connect(uri: &str, credentials: Credentials, options: ConnectionProperties) -> Confirmation<Connection> {
    Connect::connect(uri, credentials, options)
  }

  /// Connect to an AMQP Server
  pub fn connect_uri(uri: AMQPUri, credentials: Credentials, options: ConnectionProperties) -> Confirmation<Connection> {
    Connect::connect(uri, credentials, options)
  }

  pub fn create_channel(&self) -> Confirmation<Channel> {
    match self.channels.create(self.clone()) {
      Ok(channel) => channel.channel_open(),
      Err(error)  => Confirmation::new_error(error),
    }
  }

  fn connector(credentials: Credentials, options: ConnectionProperties) -> impl FnOnce(TcpStream, AMQPUri) -> Result<(Wait<Connection>, IoLoop<TcpStream>), Error> + 'static {
    move |stream, uri| {
      let conn = Connection::default();
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
      conn.status.set_state(ConnectionState::SentProtocolHeader(wait_handle, credentials, options));
      let io_loop = IoLoop::new(conn.clone(), stream)?;
      // FIXME: how do we bubble errors up during connection?
      Ok((wait, io_loop))
    }
  }

  pub fn set_vhost(&self, vhost: &str) {
    self.status.set_vhost(vhost);
  }

  fn set_readable(&self) -> Result<(), Error> {
    trace!("connection set readable");
    self.registration.set_readiness(Ready::readable()).map_err(ErrorKind::IOError)?;
    Ok(())
  }

  pub fn send_frame(&self, channel_id: u16, priority: Priority, frame: AMQPFrame, expected_reply: Option<Reply>) -> Result<Wait<()>, Error> {
    trace!("connection send_frame; channel_id={}", channel_id);
    let wait = self.frames.push(channel_id, priority, frame, expected_reply);
    self.set_readable()?;
    Ok(wait)
  }

  pub fn next_expected_reply(&self, channel_id: u16) -> Option<Reply> {
    self.frames.next_expected_reply(channel_id)
  }

  /// next message to send to the network
  ///
  /// returns None if there's no message to send
  pub fn next_frame(&self) -> Option<(SendId, AMQPFrame)> {
    self.frames.pop()
  }

  /// updates the current state with a new received frame
  pub fn handle_frame(&self, f: AMQPFrame) -> Result<(), Error> {
    trace!("will handle frame: {:?}", f);
    match f {
      AMQPFrame::ProtocolHeader => {
        error!("error: the client should not receive a protocol header");
        self.set_error()?;
      },
      AMQPFrame::Method(channel_id, method) => {
        self.channels.receive_method(channel_id, method)?;
      },
      AMQPFrame::Heartbeat(_) => {
        debug!("received heartbeat from server");
      },
      AMQPFrame::Header(channel_id, _, header) => {
        self.channels.handle_content_header_frame(channel_id, header.body_size, header.properties)?;
      },
      AMQPFrame::Body(channel_id, payload) => {
        self.channels.handle_body_frame(channel_id, payload)?;
      }
    };
    Ok(())
  }

  pub fn send_heartbeat(&self) -> Result<(), Error> {
    self.set_readable()?;
    self.send_frame(0, Priority::CRITICAL, AMQPFrame::Heartbeat(0), None)?;
    Ok(())
  }

  #[doc(hidden)]
  pub fn requeue_frame(&self, send_id: SendId, frame: AMQPFrame) -> Result<(), Error> {
    self.set_readable()?;
    self.frames.retry(send_id, frame);
    Ok(())
  }

  #[doc(hidden)]
  pub fn has_pending_frames(&self) -> bool {
    !self.frames.is_empty()
  }

  pub fn mark_sent(&self, send_id: SendId) {
    self.frames.mark_sent(send_id);
  }

  pub fn remove_channel(&self, channel_id: u16) -> Result<(), Error> {
    self.frames.clear_expected_replies(channel_id);
    self.channels.remove(channel_id)
  }

  pub fn set_closing(&self) {
    self.status.set_state(ConnectionState::Closing);
    self.channels.set_closing();
  }

  pub fn set_closed(&self) -> Result<(), Error> {
    self.status.set_state(ConnectionState::Closed);
    self.channels.set_closed()
  }

  pub fn set_error(&self) -> Result<(), Error> {
    error!("Connection error");
    self.status.set_state(ConnectionState::Error);
    self.channels.set_error()
  }
}

/// Trait providing a method to connect to an AMQP server
pub trait Connect {
  /// connect to an AMQP server
  fn connect(self, credentials: Credentials, options: ConnectionProperties) -> Confirmation<Connection> where Self: Sized {
    self.connect_raw(credentials, options).into()
  }

  /// connect to an AMQP server, for internal use
  fn connect_raw(self, credentials: Credentials, options: ConnectionProperties) -> Result<Wait<Connection>, Error>;
}

impl Connect for AMQPUri {
  fn connect_raw(self, credentials: Credentials, options: ConnectionProperties) -> Result<Wait<Connection>, Error> {
    let (conn, io_loop) = AMQPUriTcpExt::connect(self, Connection::connector(credentials, options)).map_err(ErrorKind::IOError)??;
    io_loop.run()?;
    Ok(conn)
  }
}

impl Connect for &str {
  fn connect_raw(self, credentials: Credentials, options: ConnectionProperties) -> Result<Wait<Connection>, Error> {
    let (conn, io_loop) = AMQPUriTcpExt::connect(self, Connection::connector(credentials, options)).map_err(ErrorKind::IOError)??;
    io_loop.run()?;
    Ok(conn)
  }
}

#[cfg(test)]
mod tests {
  use env_logger;

  use super::*;
  use crate::channel::BasicProperties;
  use crate::channel_status::ChannelState;
  use crate::consumer::ConsumerSubscriber;
  use crate::message::Delivery;
  use crate::types::ShortString;
  use amq_protocol::protocol::{basic, AMQPClass};
  use amq_protocol::frame::AMQPContentHeader;

  #[derive(Clone,Debug,PartialEq)]
  struct DummySubscriber;

  impl ConsumerSubscriber for DummySubscriber {
    fn new_delivery(&self, _delivery: Delivery) {}
    fn drop_prefetched_messages(&self) {}
    fn cancel(&self) {}
  }

  #[test]
  fn basic_consume_small_payload() {
    let _ = env_logger::try_init();

    use crate::consumer::Consumer;
    use crate::queue::{Queue, QueueState};

    // Bootstrap connection state to a consuming state
    let conn = Connection::default();
    conn.status.set_state(ConnectionState::Connected);
    conn.configuration.set_channel_max(2047);
    let channel = conn.channels.create(conn.clone()).unwrap();
    channel.status.set_state(ChannelState::Connected);
    let queue_name = ShortString::from("consumed");
    let mut queue: QueueState = Queue::new(queue_name.clone(), 0, 0).into();
    let consumer_tag = ShortString::from("consumer-tag");
    let consumer = Consumer::new(consumer_tag.clone(), false, false, false, Box::new(DummySubscriber));
    queue.consumers.insert(consumer_tag.clone(), consumer);
    conn.channels.get(channel.id()).map(|c| {
      c.queues.register(queue);
    });
    // Now test the state machine behaviour
    {
      let deliver_frame = AMQPFrame::Method(
        channel.id(),
        AMQPClass::Basic(
          basic::AMQPMethod::Deliver(
            basic::Deliver {
              consumer_tag: consumer_tag.clone(),
              delivery_tag: 1,
              redelivered: false,
              exchange: "".into(),
              routing_key: queue_name.clone(),
            }
          )
        )
      );
      conn.handle_frame(deliver_frame).unwrap();
      let channel_state = channel.status.state();
      let expected_state = ChannelState::WillReceiveContent(
        Some(queue_name.clone()),
        Some(consumer_tag.clone())
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
        })
      );
      conn.handle_frame(header_frame).unwrap();
      let channel_state = channel.status.state();
      let expected_state = ChannelState::ReceivingContent(Some(queue_name.clone()), Some(consumer_tag.clone()), 2);
      assert_eq!(channel_state, expected_state);
    }
    {
      let body_frame = AMQPFrame::Body(channel.id(), "{}".as_bytes().to_vec());
      conn.handle_frame(body_frame).unwrap();
      let channel_state = channel.status.state();
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
    conn.status.set_state(ConnectionState::Connected);
    conn.configuration.set_channel_max(2047);
    let channel = conn.channels.create(conn.clone()).unwrap();
    channel.status.set_state(ChannelState::Connected);
    let queue_name = ShortString::from("consumed");
    let mut queue: QueueState = Queue::new(queue_name.clone(), 0, 0).into();
    let consumer_tag = ShortString::from("consumer-tag");
    let consumer = Consumer::new(consumer_tag.clone(), false, false, false, Box::new(DummySubscriber));
    queue.consumers.insert(consumer_tag.clone(), consumer);
    conn.channels.get(channel.id()).map(|c| {
      c.queues.register(queue);
    });
    // Now test the state machine behaviour
    {
      let deliver_frame = AMQPFrame::Method(
        channel.id(),
        AMQPClass::Basic(
          basic::AMQPMethod::Deliver(
            basic::Deliver {
              consumer_tag: consumer_tag.clone(),
              delivery_tag: 1,
              redelivered: false,
              exchange: "".into(),
              routing_key: queue_name.clone(),
            }
          )
        )
      );
      conn.handle_frame(deliver_frame).unwrap();
      let channel_state = channel.status.state();
      let expected_state = ChannelState::WillReceiveContent(
        Some(queue_name.clone()),
        Some(consumer_tag.clone())
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
        })
      );
      conn.handle_frame(header_frame).unwrap();
      let channel_state = channel.status.state();
      let expected_state = ChannelState::Connected;
      assert_eq!(channel_state, expected_state);
    }
  }
}
