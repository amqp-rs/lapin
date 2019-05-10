use amq_protocol::frame::{AMQPFrame, GenError, Offset, gen_frame, parse_frame};
use log::{debug, error, trace};

use crate::{
  channel::Channel,
  channels::Channels,
  configuration::Configuration,
  connection_properties::ConnectionProperties,
  connection_status::{ConnectionStatus, ConnectionState, ConnectingState},
  credentials::Credentials,
  error::{Error, ErrorKind},
  frames::Frames,
};

#[derive(Clone, Debug)]
pub struct Connection {
  /// current state of the connection. In normal use it should always be ConnectionState::Connected
  pub status:          ConnectionStatus,
  pub channels:        Channels,
  pub configuration:   Configuration,
  // list of frames to send
      frames:          Frames,
}

impl Default for Connection {
  fn default() -> Self {
    let configuration = Configuration::default();
    let channels = Channels::default();

    let connection = Self {
      status:          ConnectionStatus::default(),
      channels,
      configuration,
      frames:          Frames::default(),
    };

    connection.channels.create_zero(connection.clone());
    connection
  }
}

impl Connection {
  /// creates a `Connection` object in initial state
  pub fn new() -> Self {
    Default::default()
  }

  pub fn create_channel(&self) -> Result<Channel, Error> {
    self.channels.create(self.clone())
  }

  /// starts the process of connecting to the server
  ///
  /// this will set up the state machine and generates the required messages.
  /// The messages will not be sent until calls to `serialize`
  /// to write the messages to a buffer, or calls to `next_frame`
  /// to obtain the next message to send
  pub fn connect(&self, credentials: Credentials, options: ConnectionProperties) -> Result<ConnectionState, Error> {
    let state = self.status.state();
    if state == ConnectionState::Initial {
      self.send_frame(AMQPFrame::ProtocolHeader);
      self.status.set_connecting_state(ConnectingState::SentProtocolHeader(credentials, options));
      Ok(self.status.state())
    } else {
      self.set_error()?;
      Err(ErrorKind::InvalidConnectionState(state).into())
    }
  }

  pub fn send_frame(&self, frame: AMQPFrame) {
    self.frames.push(frame);
  }

  /// next message to send to the network
  ///
  /// returns None if there's no message to send
  pub fn next_frame(&self) -> Option<AMQPFrame> {
    self.frames.pop()
  }

  /// writes the next message to a mutable byte slice
  ///
  /// returns how many bytes were written and the current state.
  /// this method can be called repeatedly until the buffer is full or
  /// there are no more frames to send
  pub fn serialize(&self, send_buffer: &mut [u8]) -> Result<(usize, ConnectionState), Error> {
    if let Some(next_msg) = self.next_frame() {
      trace!("will write to buffer: {:?}", next_msg);
      match gen_frame((send_buffer, 0), &next_msg).map(|tup| tup.1) {
        Ok(sz) => {
          Ok((sz, self.status.state()))
        },
        Err(e) => {
          error!("error generating frame: {:?}", e);
          self.set_error()?;
          match e {
            GenError::BufferTooSmall(_) => {
              // Requeue msg
              self.requeue_frame(next_msg);
              Err(ErrorKind::SendBufferTooSmall.into())
            },
            GenError::InvalidOffset | GenError::CustomError(_) | GenError::NotYetImplemented => {
              Err(ErrorKind::SerialisationError(e).into())
            }
          }
        }
      }
    } else {
      Err(ErrorKind::NoNewMessage.into())
    }
  }

  /// parses a frame from a byte slice
  ///
  /// returns how many bytes were consumed and the current state.
  ///
  /// This method will update the state machine according to the ReceivedStart
  /// frame with `handle_frame`
  pub fn parse(&self, data: &[u8]) -> Result<(usize,ConnectionState), Error> {
    match parse_frame(data) {
      Ok((i, f)) => {
        let consumed = data.offset(i);

        if let Err(e) = self.handle_frame(f) {
          self.set_error()?;
          Err(e)
        } else {
          Ok((consumed, self.status.state()))
        }
      },
      Err(e) => {
        if e.is_incomplete() {
          Ok((0,self.status.state()))
        } else {
          self.set_error()?;
          Err(ErrorKind::ParsingError(format!("{:?}", e)).into())
        }
      }
    }
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

  #[doc(hidden)]
  pub fn send_preemptive_frame(&self, frame: AMQPFrame) {
    self.frames.push_preemptive(frame);
  }

  #[doc(hidden)]
  pub fn requeue_frame(&self, frame: AMQPFrame) {
    self.frames.retry(frame);
  }

  #[doc(hidden)]
  pub fn has_pending_frames(&self) -> bool {
    !self.frames.is_empty()
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
    self.status.set_state(ConnectionState::Error);
    self.channels.set_error()
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
    use crate::queue::Queue;

    // Bootstrap connection state to a consuming state
    let conn = Connection::new();
    conn.status.set_state(ConnectionState::Connected);
    conn.configuration.set_channel_max(2047);
    let channel = conn.create_channel().unwrap();
    channel.status.set_state(ChannelState::Connected);
    let queue_name = "consumed".to_string();
    let mut queue = Queue::new(queue_name.clone(), 0, 0);
    let consumer_tag = "consumer-tag".to_string();
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
              exchange: "".to_string(),
              routing_key: queue_name.clone(),
            }
          )
        )
      );
      conn.handle_frame(deliver_frame).unwrap();
      let channel_state = channel.status.state();
      let expected_state = ChannelState::WillReceiveContent(
        queue_name.clone(),
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
      let expected_state = ChannelState::ReceivingContent(queue_name.clone(), Some(consumer_tag.clone()), 2);
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
    use crate::queue::Queue;

    // Bootstrap connection state to a consuming state
    let conn = Connection::new();
    conn.status.set_state(ConnectionState::Connected);
    conn.configuration.set_channel_max(2047);
    let channel = conn.create_channel().unwrap();
    channel.status.set_state(ChannelState::Connected);
    let queue_name = "consumed".to_string();
    let mut queue = Queue::new(queue_name.clone(), 0, 0);
    let consumer_tag = "consumer-tag".to_string();
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
              exchange: "".to_string(),
              routing_key: queue_name.clone(),
            }
          )
        )
      );
      conn.handle_frame(deliver_frame).unwrap();
      let channel_state = channel.status.state();
      let expected_state = ChannelState::WillReceiveContent(
        queue_name.clone(),
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
