use amq_protocol::frame::{AMQPFrame, GenError, Offset, gen_frame, parse_frame};
use amq_protocol::protocol::{AMQPClass, connection};
use crossbeam_channel::{self, Sender, Receiver};
use log::{debug, error, trace, warn};

use std::result;
use std::io::{Error, ErrorKind, Result};

use crate::{
  channels::Channels,
  configuration::Configuration,
  connection_properties::ConnectionProperties,
  connection_status::{ConnectionStatus, ConnectionState, ConnectingState},
  credentials::Credentials,
  error,
  priority_frames::PriorityFrames,
  types::{AMQPValue, FieldTable},
};

#[derive(Clone, Debug)]
pub struct Connection {
  /// current state of the connection. In normal use it should always be ConnectionState::Connected
  pub status:            ConnectionStatus,
  pub channels:          Channels,
  pub configuration:     Configuration,
  /// list of message to send
      frame_receiver:    Receiver<AMQPFrame>,
  /// We keep a copy so that it never gets shutdown and to create new channels
      frame_sender:      Sender<AMQPFrame>,
  /// Failed frames we need to try and send back + heartbeats
      priority_frames:   PriorityFrames,
}

impl Default for Connection {
  fn default() -> Self {
    let (sender, receiver) = crossbeam_channel::unbounded();
    let configuration = Configuration::default();

    Self {
      status:            ConnectionStatus::default(),
      channels:          Channels::new(configuration.clone(), sender.clone()),
      configuration,
      frame_receiver:    receiver,
      frame_sender:      sender,
      priority_frames:   PriorityFrames::default(),
    }
  }
}

impl Connection {
  /// creates a `Connection` object in initial state
  pub fn new() -> Self {
    Default::default()
  }

  /// starts the process of connecting to the server
  ///
  /// this will set up the state machine and generates the required messages.
  /// The messages will not be sent until calls to `serialize`
  /// to write the messages to a buffer, or calls to `next_frame`
  /// to obtain the next message to send
  pub fn connect(&mut self, credentials: Credentials, options: ConnectionProperties) -> Result<ConnectionState> {
    let state = self.status.state();
    if state != ConnectionState::Initial {
      self.status.set_error();
      return Err(Error::new(ErrorKind::Other, format!("invalid state: {:?}", state)));
    }

    self.channels.send_frame(0, AMQPFrame::ProtocolHeader).expect("channel 0");
    self.status.set_connecting_state(ConnectingState::SentProtocolHeader(credentials, options));
    Ok(self.status.state())
  }

  /// next message to send to the network
  ///
  /// returns None if there's no message to send
  pub fn next_frame(&mut self) -> Option<AMQPFrame> {
    self.priority_frames.pop().or_else(|| {
      // Error means no message
      self.frame_receiver.try_recv().ok()
    })
  }

  /// writes the next message to a mutable byte slice
  ///
  /// returns how many bytes were written and the current state.
  /// this method can be called repeatedly until the buffer is full or
  /// there are no more frames to send
  pub fn serialize(&mut self, send_buffer: &mut [u8]) -> Result<(usize, ConnectionState)> {
    if let Some(next_msg) = self.next_frame() {
      trace!("will write to buffer: {:?}", next_msg);
      match gen_frame((send_buffer, 0), &next_msg).map(|tup| tup.1) {
        Ok(sz) => {
          Ok((sz, self.status.state()))
        },
        Err(e) => {
          error!("error generating frame: {:?}", e);
          self.status.set_error();
          match e {
            GenError::BufferTooSmall(_) => {
              // Requeue msg
              self.requeue_frame(next_msg);
              Err(Error::new(ErrorKind::InvalidData, "send buffer too small"))
            },
            GenError::InvalidOffset | GenError::CustomError(_) | GenError::NotYetImplemented => {
              Err(Error::new(ErrorKind::InvalidData, "could not generate"))
            }
          }
        }
      }
    } else {
      Err(Error::new(ErrorKind::WouldBlock, "no new message"))
    }
  }

  /// parses a frame from a byte slice
  ///
  /// returns how many bytes were consumed and the current state.
  ///
  /// This method will update the state machine according to the ReceivedStart
  /// frame with `handle_frame`
  pub fn parse(&mut self, data: &[u8]) -> Result<(usize,ConnectionState)> {
    match parse_frame(data) {
      Ok((i, f)) => {
        let consumed = data.offset(i);

        if let Err(e) = self.handle_frame(f) {
          self.status.set_error();
          Err(Error::new(ErrorKind::Other, format!("failed to handle frame: {:?}", e)))
        } else {
          Ok((consumed, self.status.state()))
        }
      },
      Err(e) => {
        if e.is_incomplete() {
          Ok((0,self.status.state()))
        } else {
          self.status.set_error();
          Err(Error::new(ErrorKind::Other, format!("parse error: {:?}", e)))
        }
      }
    }
  }

  /// updates the current state with a new received frame
  pub fn handle_frame(&mut self, f: AMQPFrame) -> result::Result<(), error::Error> {
    trace!("will handle frame: {:?}", f);
    match f {
      AMQPFrame::ProtocolHeader => {
        error!("error: the client should not receive a protocol header");
        self.status.set_error();
      },
      AMQPFrame::Method(channel_id, method) => {
        if channel_id == 0 {
          self.handle_global_method(method);
        } else {
          self.channels.receive_method(channel_id, method)?;
        }
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
  #[clippy::cyclomatic_complexity = "40"]
  pub fn handle_global_method(&mut self, c: AMQPClass) {
    let state = self.status.state();
    match state {
      ConnectionState::Initial | ConnectionState::Closed | ConnectionState::Error => {
        error!("Received method in global channel and we're {:?}: {:?}", state, c);
        self.status.set_error();
      },
      ConnectionState::Connecting(connecting_state) => {
        match connecting_state {
          ConnectingState::Initial => {
            error!("Received method in global channel and we're Conntecting/Initial: {:?}", c);
            self.status.set_error()
          },
          ConnectingState::SentProtocolHeader(credentials, mut options) => {
            if let AMQPClass::Connection(connection::AMQPMethod::Start(s)) = c {
              trace!("Server sent Connection::Start: {:?}", s);
              self.status.set_connecting_state(ConnectingState::ReceivedStart);

              let mechanism = options.mechanism.to_string();
              let locale    = options.locale.clone();

              if !s.mechanisms.split_whitespace().any(|m| m == mechanism) {
                error!("unsupported mechanism: {}", mechanism);
              }
              if !s.locales.split_whitespace().any(|l| l == locale) {
                error!("unsupported locale: {}", mechanism);
              }

              if !options.client_properties.contains_key("product") || !options.client_properties.contains_key("version") {
                options.client_properties.insert("product".to_string(), AMQPValue::LongString(env!("CARGO_PKG_NAME").to_string()));
                options.client_properties.insert("version".to_string(), AMQPValue::LongString(env!("CARGO_PKG_VERSION").to_string()));
              }

              options.client_properties.insert("platform".to_string(), AMQPValue::LongString("rust".to_string()));

              let mut capabilities = FieldTable::new();
              capabilities.insert("publisher_confirms".to_string(), AMQPValue::Boolean(true));
              capabilities.insert("exchange_exchange_bindings".to_string(), AMQPValue::Boolean(true));
              capabilities.insert("basic.nack".to_string(), AMQPValue::Boolean(true));
              capabilities.insert("consumer_cancel_notify".to_string(), AMQPValue::Boolean(true));
              capabilities.insert("connection.blocked".to_string(), AMQPValue::Boolean(true));
              capabilities.insert("authentication_failure_close".to_string(), AMQPValue::Boolean(true));

              options.client_properties.insert("capabilities".to_string(), AMQPValue::FieldTable(capabilities));

              let start_ok = AMQPClass::Connection(connection::AMQPMethod::StartOk(
                  connection::StartOk {
                    client_properties: options.client_properties,
                    mechanism,
                    locale,
                    response:          credentials.sasl_plain_auth_string()
                  }
              ));

              debug!("client sending Connection::StartOk: {:?}", start_ok);
              self.channels.send_method_frame(0, start_ok).expect("channel 0");
              self.status.set_connecting_state(ConnectingState::SentStartOk);
            } else {
              trace!("waiting for class Connection method Start, got {:?}", c);
              self.status.set_error();
            }
          },
          ConnectingState::ReceivedStart => {
            error!("state {:?}\treceived\t{:?}", self.status.state(), c);
            self.status.set_error()
          },
          ConnectingState::SentStartOk => {
            if let AMQPClass::Connection(connection::AMQPMethod::Tune(t)) = c {
              debug!("Server sent Connection::Tune: {:?}", t);
              self.status.set_connecting_state(ConnectingState::ReceivedTune);

              // If we disable the heartbeat (0) but the server don't, follow him and enable it too
              // If both us and the server want heartbeat enabled, pick the lowest value.
              if self.configuration.heartbeat() == 0 || t.heartbeat != 0 && t.heartbeat < self.configuration.heartbeat() {
                self.configuration.set_heartbeat(t.heartbeat);
              }

              if t.channel_max != 0 {
                // 0 means we want to take the server's value
                // If both us and the server specified a channel_max, pick the lowest value.
                if self.configuration.channel_max() == 0 || t.channel_max < self.configuration.channel_max() {
                  self.configuration.set_channel_max(t.channel_max);
                }
              }
              if self.configuration.channel_max() == 0 {
                self.configuration.set_channel_max(u16::max_value());
              }

              if t.frame_max != 0 {
                // 0 means we want to take the server's value
                // If both us and the server specified a frame_max, pick the lowest value.
                if self.configuration.frame_max() == 0 || t.frame_max < self.configuration.frame_max() {
                  self.configuration.set_frame_max(t.frame_max);
                }
              }
              if self.configuration.frame_max() == 0 {
                self.configuration.set_frame_max(u32::max_value());
              }

              let tune_ok = AMQPClass::Connection(connection::AMQPMethod::TuneOk(
                  connection::TuneOk {
                    channel_max: self.configuration.channel_max(),
                    frame_max:   self.configuration.frame_max(),
                    heartbeat:   self.configuration.heartbeat(),
                  }
              ));

              debug!("client sending Connection::TuneOk: {:?}", tune_ok);

              self.channels.send_method_frame(0, tune_ok).expect("channel 0");
              self.status.set_connecting_state(ConnectingState::SentTuneOk);

              let open = AMQPClass::Connection(connection::AMQPMethod::Open(connection::Open { virtual_host: self.status.vhost() }));

              debug!("client sending Connection::Open: {:?}", open);
              self.channels.send_method_frame(0, open).expect("channel 0");
              self.status.set_connecting_state(ConnectingState::SentOpen);
            } else {
              trace!("waiting for class Connection method Start, got {:?}", c);
              self.status.set_error();
            }
          },
          ConnectingState::ReceivedTune => {
            error!("state {:?}\treceived\t{:?}", self.status.state(), c);
            self.status.set_error()
          },
          ConnectingState::SentTuneOk => {
            error!("state {:?}\treceived\t{:?}", self.status.state(), c);
            self.status.set_error()
          },
          ConnectingState::SentOpen => {
            trace!("state {:?}\treceived\t{:?}", self.status.state(), c);
            if let AMQPClass::Connection(connection::AMQPMethod::OpenOk(o)) = c {
              debug!("Server sent Connection::OpenOk: {:?}, client now connected", o);
              self.status.set_state(ConnectionState::Connected);
            } else {
              trace!("waiting for class Connection method Start, got {:?}", c);
              self.status.set_error();
            }
          },
          ConnectingState::ReceivedSecure => {
            error!("state {:?}\treceived\t{:?}", self.status.state(), c);
            self.status.set_error();
          },
          ConnectingState::SentSecure => {
            error!("state {:?}\treceived\t{:?}", self.status.state(), c);
            self.status.set_error();
          },
          ConnectingState::ReceivedSecondSecure => {
            error!("state {:?}\treceived\t{:?}", self.status.state(), c);
            self.status.set_error();
          },
          ConnectingState::Error => {
            error!("state {:?}\treceived\t{:?}", self.status.state(), c);
          },
        }
      },
      ConnectionState::Connected => {
        warn!("Ignoring method from global channel as we are connected {:?}", c);
      },
      ConnectionState::Closing(_) => {
        warn!("Ignoring method from global channel as we are closing: {:?}", c);
      },
    };
  }

  #[doc(hidden)]
  pub fn send_preemptive_frame(&mut self, frame: AMQPFrame) {
    self.priority_frames.push_front(frame);
  }

  #[doc(hidden)]
  pub fn requeue_frame(&mut self, frame: AMQPFrame) {
    self.priority_frames.push_back(frame);
  }

  #[doc(hidden)]
  pub fn has_pending_frames(&self) -> bool {
    !self.priority_frames.is_empty() || !self.frame_receiver.is_empty()
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
  use amq_protocol::protocol::basic;
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
    let mut conn = Connection::new();
    conn.status.set_state(ConnectionState::Connected);
    conn.configuration.set_channel_max(2047);
    let channel = conn.channels.create().unwrap();
    channel.status.set_state(ChannelState::Connected);
    let queue_name = "consumed".to_string();
    let mut queue = Queue::new(queue_name.clone(), 0, 0);
    let consumer_tag = "consumer-tag".to_string();
    let consumer = Consumer::new(consumer_tag.clone(), false, false, false, false, Box::new(DummySubscriber));
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
    let mut conn = Connection::new();
    conn.status.set_state(ConnectionState::Connected);
    conn.configuration.set_channel_max(2047);
    let channel = conn.channels.create().unwrap();
    channel.status.set_state(ChannelState::Connected);
    let queue_name = "consumed".to_string();
    let mut queue = Queue::new(queue_name.clone(), 0, 0);
    let consumer_tag = "consumer-tag".to_string();
    let consumer = Consumer::new(consumer_tag.clone(), false, false, false, false, Box::new(DummySubscriber));
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
