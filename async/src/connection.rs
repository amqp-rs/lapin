use amq_protocol::frame::{AMQPContentHeader, AMQPFrame, gen_frame, parse_frame};
use amq_protocol::protocol::{AMQPClass, connection};
use amq_protocol::sasl;
use crossbeam_channel::{self, Sender, Receiver};
use cookie_factory::GenError;
use log::{debug, error, trace};
use nom::Offset;
use parking_lot::Mutex;

use std::{fmt, result, str};
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;

use crate::api::{Answer, ChannelState, RequestId};
use crate::channel::{Channel, BasicProperties};
use crate::error;
use crate::message::BasicGetMessage;
use crate::types::{AMQPValue, FieldTable};

#[derive(Clone,Debug,PartialEq)]
pub enum ConnectionState {
  Initial,
  Connecting(ConnectingState),
  Connected,
  Closing(ClosingState),
  Closed,
  Error,
}

#[derive(Clone,Debug,PartialEq)]
pub enum ConnectingState {
  Initial,
  SentProtocolHeader(ConnectionProperties),
  ReceivedStart,
  SentStartOk,
  ReceivedSecure,
  SentSecure,
  ReceivedSecondSecure,
  ReceivedTune,
  SentTuneOk,
  SentOpen,
  Error,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ClosingState {
  Initial,
  SentClose,
  ReceivedClose,
  SentCloseOk,
  ReceivedCloseOk,
  Error,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ConnectionSASLMechanism {
  PLAIN,
}

impl fmt::Display for ConnectionSASLMechanism {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct ConnectionProperties {
  pub mechanism:         ConnectionSASLMechanism,
  pub locale:            String,
  pub client_properties: FieldTable,
}

impl Default for ConnectionProperties {
  fn default() -> Self {
    Self {
      mechanism:         ConnectionSASLMechanism::PLAIN,
      locale:            "en_US".to_string(),
      client_properties: FieldTable::new(),
    }
  }
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct Configuration {
  pub channel_max: u16,
  pub frame_max:   u32,
  pub heartbeat:   u16,
}

#[derive(Clone,Debug,PartialEq)]
pub struct Credentials {
  username: String,
  password: String,
}

impl Default for Credentials {
  fn default() -> Credentials {
    Credentials {
      username: "guest".to_string(),
      password: "guest".to_string(),
    }
  }
}

#[derive(Debug)]
pub struct Connection {
  /// current state of the connection. In normal use it should always be ConnectionState::Connected
  pub state:             ConnectionState,
  pub channels:          HashMap<u16, Channel>,
  pub configuration:     Configuration,
  pub vhost:             String,
  pub channel_index:     u16,
  pub channel_id_lock:   Arc<Mutex<()>>,
  pub prefetch_size:     u32,
  pub prefetch_count:    u16,
  /// list of message to send
  pub frame_receiver:    Receiver<AMQPFrame>,
  // We keep a copy so that it never gets shutdown and to create new channels
      frame_sender:      Sender<AMQPFrame>,
  /// next request id
  pub request_index:     RequestId,
  /// list of finished requests
  /// value is true if the request returned something or false otherwise
  pub finished_reqs:     HashMap<RequestId, bool>,
  /// list of finished basic get requests
  /// value is true if the request returned something or false otherwise
  pub finished_get_reqs: HashMap<RequestId, bool>,
  /// list of generated names (e.g. when supplying empty string for consumer tag or queue name)
  pub generated_names:   HashMap<RequestId, String>,
  /// credentials are stored in an option to remove them from memory once they are used
  pub credentials:       Option<Credentials>,
}

impl Connection {
  /// creates a `Connection` object in initial state
  pub fn new() -> Connection {
    let mut h = HashMap::new();
    h.insert(0, Channel::global());

    let (sender, receiver) = crossbeam_channel::unbounded();

    Connection {
      state:             ConnectionState::Initial,
      channels:          h,
      configuration:     Configuration::default(),
      vhost:             "/".to_string(),
      channel_index:     0,
      channel_id_lock:   Arc::new(Mutex::new(())),
      prefetch_size:     0,
      prefetch_count:    0,
      frame_receiver:    receiver,
      frame_sender:      sender,
      request_index:     0,
      finished_reqs:     HashMap::new(),
      finished_get_reqs: HashMap::new(),
      generated_names:   HashMap::new(),
      credentials:       None,
    }
  }

  pub fn set_credentials(&mut self, username: &str, password: &str) {
    self.credentials = Some(Credentials {
      username: username.to_string(),
      password: password.to_string(),
    });
  }

  pub fn set_vhost(&mut self, vhost: &str) {
    self.vhost = vhost.to_string();
  }

  pub fn set_heartbeat(&mut self, heartbeat: u16) {
    self.configuration.heartbeat = heartbeat;
  }

  pub fn set_channel_max(&mut self, channel_max: u16) {
    self.configuration.channel_max = channel_max;
  }

  pub fn set_frame_max(&mut self, frame_max: u32) {
    self.configuration.frame_max = frame_max;
  }

  /// creates a `Channel` object in initial state
  ///
  /// returns a `u16` channel id
  ///
  /// The channel will not be usable until `channel_open`
  /// is called with the channel id
  pub fn create_channel(&mut self) -> Option<u16> {
    let _lock  = self.channel_id_lock.lock();
    let offset = if self.channel_index == self.configuration.channel_max {
      // skip 0 and go straight to 1
      1
    } else {
      self.channel_index + 1
    };

    let id = (offset..self.configuration.channel_max).chain(1..offset).find(|id| {
      self.channels.get(&id).map(|channel| !channel.is_connected()).unwrap_or(true)
    })?;

    let c = Channel::new(id);
    self.channel_index = id;
    self.channels.insert(id, c);
    Some(id)
  }

  pub fn set_channel_state(&mut self, channel_id: u16, new_state: ChannelState) {
    self.channels.get_mut(&channel_id).map(|c| c.state = new_state);
  }

  /// verifies if the channel's state is the one passed as argument
  ///
  /// returns a Option of the result. None in the case the channel
  /// does not exists
  pub fn check_state(&self, channel_id: u16, state: ChannelState) -> result::Result<(), error::Error> {
    self.channels
      .get(&channel_id)
      .map_or(Err(error::ErrorKind::InvalidChannel(channel_id).into()), |c| {
        if c.state == state {
          Ok(())
        } else {
          Err(error::ErrorKind::InvalidState {
            expected: state,
            actual:   c.state.clone(),
          }.into())
        }
      })
  }

  /// returns the channel's state
  ///
  /// returns a Option of the state. Non in the case the channel
  /// does not exists
  pub fn get_state(&self, channel_id: u16) -> Option<ChannelState> {
    self.channels
      .get(&channel_id)
      .map(|c| c.state.clone())
  }

  #[doc(hidden)]
  pub fn push_back_answer(&mut self, channel_id: u16, answer: Answer) {
    self.channels
      .get_mut(&channel_id)
      .map(|c| c.awaiting.push_back(answer));
  }

  #[doc(hidden)]
  pub fn get_next_answer(&mut self, channel_id: u16) -> Option<Answer> {
    self.channels
      .get_mut(&channel_id)
      .and_then(|c| c.awaiting.pop_front())
  }

  /// verifies if the channel is connecyed
  pub fn is_connected(&self, channel_id: u16) -> bool {
    self.channels
      .get(&channel_id)
      .map(|c| c.is_connected()).unwrap_or(false)
  }

  #[doc(hidden)]
  pub fn next_request_id(&mut self) -> RequestId {
    let id = self.request_index;
    self.request_index += 1;
    id
  }

  /// Get the name generated by the server for a given `RequestId`
  ///
  /// this method can only be called once per request id, as it will be
  /// removed from the list afterwards
  pub fn get_generated_name(&mut self, id: RequestId) -> Option<String> {
    self.generated_names.remove(&id)
  }

  /// verifies if the request identified with the `RequestId` is finished
  ///
  /// this method can only be called once per request id, as it will be
  /// removed from the list afterwards
  pub fn is_finished(&mut self, id: RequestId) -> Option<bool> {
    self.finished_reqs.remove(&id)
  }

  /// verifies if the get request identified with the `RequestId` is finished
  ///
  /// this method can only be called once per request id, as it will be
  /// removed from the list afterwards
  pub fn finished_get_result(&mut self, id: RequestId) -> Option<bool> {
    self.finished_get_reqs.remove(&id)
  }

  /// gets the next message corresponding to a channel and queue, in response to a basic.get
  ///
  /// if the channel id and queue have no link, the method
  /// will return None. If there is no message, the method will return None
  pub fn next_basic_get_message(&mut self, channel_id: u16, queue_name: &str) -> Option<BasicGetMessage> {
    self.channels.get_mut(&channel_id)
      .and_then(|channel| channel.queues.get_mut(queue_name))
      .and_then(|queue| queue.next_basic_get_message())
  }

  /// starts the process of connecting to the server
  ///
  /// this will set up the state machine and generates the required messages.
  /// The messages will not be sent until calls to `serialize`
  /// to write the messages to a buffer, or calls to `next_frame`
  /// to obtain the next message to send
  pub fn connect(&mut self, options: ConnectionProperties) -> Result<ConnectionState> {
    if self.state != ConnectionState::Initial {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    self.send_frame(AMQPFrame::ProtocolHeader);
    self.state = ConnectionState::Connecting(ConnectingState::SentProtocolHeader(options));
    Ok(self.state.clone())
  }

  /// next message to send to the network
  ///
  /// returns None if there's no message to send
  pub fn next_frame(&mut self) -> Option<AMQPFrame> {
    // Error means no message
    self.frame_receiver.try_recv().ok()
  }

  /// writes the next message to a mutable byte slice
  ///
  /// returns how many bytes were written and the current state.
  /// this method can be called repeatedly until the buffer is full or
  /// there are no more frames to send
  pub fn serialize(&mut self, send_buffer: &mut [u8]) -> Result<(usize, ConnectionState)> {
    let next_msg = self.next_frame();
    if next_msg == None {
      return Err(Error::new(ErrorKind::WouldBlock, "no new message"));
    }

    let next_msg = next_msg.unwrap();
    trace!("will write to buffer: {:?}", next_msg);

    let gen_res = gen_frame((send_buffer, 0), &next_msg).map(|tup| tup.1);

    match gen_res {
      Ok(sz) => {
        Ok((sz, self.state.clone()))
      },
      Err(e) => {
        error!("error generating frame: {:?}", e);
        self.state = ConnectionState::Error;
        match e {
          GenError::BufferTooSmall(_) => {
            self.send_frame(next_msg);
            return Err(Error::new(ErrorKind::InvalidData, "send buffer too small"));
          },
          GenError::InvalidOffset | GenError::CustomError(_) | GenError::NotYetImplemented => {
            return Err(Error::new(ErrorKind::InvalidData, "could not generate"));
          }
        }
      }
    }
  }

  /// parses a frame from a byte slice
  ///
  /// returns how many bytes were consumed and the current state.
  ///
  /// This method will update the state machine according to the ReceivedStart
  /// frame with `handle_frame`
  pub fn parse(&mut self, data: &[u8]) -> Result<(usize,ConnectionState)> {
    let parsed_frame = parse_frame(data);
    if let Err(e) = parsed_frame {
      if e.is_incomplete() {
        return Ok((0,self.state.clone()));
      } else {
        //FIXME: should probably disconnect on error here
        let err = format!("parse error: {:?}", e);
        self.state = ConnectionState::Error;
        return Err(Error::new(ErrorKind::Other, err))
      }
    }

    let (i, f) = parsed_frame.unwrap();

    //FIXME: what happens if we fail to parse a packet in a channel?
    // do we continue?
    let consumed = data.offset(i);

    if let Err(e) = self.handle_frame(f) {
      //FIXME: should probably disconnect on error here
      let err = format!("failed to handle frame: {:?}", e);
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, err))
    }

    return Ok((consumed, self.state.clone()));
  }

  /// updates the current state with a new received frame
  pub fn handle_frame(&mut self, f: AMQPFrame) -> result::Result<(), error::Error> {
    trace!("will handle frame: {:?}", f);
    match f {
      AMQPFrame::ProtocolHeader => {
        error!("error: the client should not receive a protocol header");
        self.state = ConnectionState::Error;
      },
      AMQPFrame::Method(channel_id, method) => {
        if channel_id == 0 {
          self.handle_global_method(method);
        } else {
          self.receive_method(channel_id, method)?;
        }
      },
      AMQPFrame::Heartbeat(_) => {
        debug!("received heartbeat from server");
      },
      AMQPFrame::Header(channel_id, _, header) => {
        self.handle_content_header_frame(channel_id, header.body_size, header.properties);
      },
      AMQPFrame::Body(channel_id, payload) => {
        self.handle_body_frame(channel_id, payload);
      }
    };
    Ok(())
  }

  #[doc(hidden)]
  pub fn handle_global_method(&mut self, c: AMQPClass) {
    match self.state.clone() {
      ConnectionState::Initial | ConnectionState::Closed | ConnectionState::Error => {
        self.state = ConnectionState::Error
      },
      ConnectionState::Connecting(connecting_state) => {
        match connecting_state {
          ConnectingState::Initial => {
            self.state = ConnectionState::Error
          },
          ConnectingState::SentProtocolHeader(mut options) => {
            if let AMQPClass::Connection(connection::AMQPMethod::Start(s)) = c {
              trace!("Server sent Connection::Start: {:?}", s);
              self.state = ConnectionState::Connecting(ConnectingState::ReceivedStart);

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

              let saved_creds = self.credentials.take().unwrap_or(Credentials::default());

              let start_ok = AMQPClass::Connection(connection::AMQPMethod::StartOk(
                  connection::StartOk {
                    client_properties: options.client_properties,
                    mechanism,
                    locale,
                    response:  sasl::plain_auth_string(&saved_creds.username, &saved_creds.password),
                  }
              ));

              debug!("client sending Connection::StartOk: {:?}", start_ok);
              self.send_method_frame(0, start_ok);
              self.state = ConnectionState::Connecting(ConnectingState::SentStartOk);
            } else {
              trace!("waiting for class Connection method Start, got {:?}", c);
              self.state = ConnectionState::Error;
            }
          },
          /*ConnectingState::ReceivedStart => {
            trace!("state {:?}\treceived\t{:?}", self.state, c);
            },*/
          ConnectingState::SentStartOk => {
            if let AMQPClass::Connection(connection::AMQPMethod::Tune(t)) = c {
              debug!("Server sent Connection::Tune: {:?}", t);
              self.state = ConnectionState::Connecting(ConnectingState::ReceivedTune);

              if self.configuration.heartbeat == 0 {
                // If we disable the heartbeat but the server don't, follow him and enable it too
                self.configuration.heartbeat = t.heartbeat;
              } else if t.heartbeat != 0 && t.heartbeat < self.configuration.heartbeat {
                // If both us and the server want heartbeat enabled, pick the lowest value.
                self.configuration.heartbeat = t.heartbeat;
              }

              if t.channel_max != 0 {
                if self.configuration.channel_max == 0 {
                  // 0 means we want to take the server's value
                  self.configuration.channel_max = t.channel_max;
                } else if t.channel_max < self.configuration.channel_max {
                  // If both us and the server specified a channel_max, pick the lowest value.
                  self.configuration.channel_max = t.channel_max;
                }
              }
              if self.configuration.channel_max == 0 {
                self.configuration.channel_max = u16::max_value();
              }

              if t.frame_max != 0 {
                if self.configuration.frame_max == 0 {
                  // 0 means we want to take the server's value
                  self.configuration.frame_max = t.frame_max;
                } else if t.frame_max < self.configuration.frame_max {
                  // If both us and the server specified a frame_max, pick the lowest value.
                  self.configuration.frame_max = t.frame_max;
                }
              }
              if self.configuration.frame_max == 0 {
                self.configuration.frame_max = u32::max_value();
              }

              let tune_ok = AMQPClass::Connection(connection::AMQPMethod::TuneOk(
                  connection::TuneOk {
                    channel_max : self.configuration.channel_max,
                    frame_max   : self.configuration.frame_max,
                    heartbeat   : self.configuration.heartbeat,
                  }
              ));

              debug!("client sending Connection::TuneOk: {:?}", tune_ok);

              self.send_method_frame(0, tune_ok);
              self.state = ConnectionState::Connecting(ConnectingState::SentTuneOk);

              let open = AMQPClass::Connection(connection::AMQPMethod::Open(
                  connection::Open {
                    virtual_host: self.vhost.clone(),
                    capabilities: "".to_string(),
                    insist:       false,
                  }
              ));

              debug!("client sending Connection::Open: {:?}", open);
              self.send_method_frame(0,open);
              self.state = ConnectionState::Connecting(ConnectingState::SentOpen);

            } else {
              trace!("waiting for class Connection method Start, got {:?}", c);
              self.state = ConnectionState::Error;
            }
          },
          ConnectingState::ReceivedSecure => {
            trace!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::SentSecure => {
            trace!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::ReceivedSecondSecure => {
            trace!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::ReceivedTune => {
            trace!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::SentOpen => {
            trace!("state {:?}\treceived\t{:?}", self.state, c);
            if let AMQPClass::Connection(connection::AMQPMethod::OpenOk(o)) = c {
              debug!("Server sent Connection::OpenOk: {:?}, client now connected", o);
              self.state = ConnectionState::Connected;
            } else {
              trace!("waiting for class Connection method Start, got {:?}", c);
              self.state = ConnectionState::Error;
            }
          },
          ConnectingState::Error => {
            trace!("state {:?}\treceived\t{:?}", self.state, c);
          },
          s => {
            error!("invalid state {:?}", s);
            self.state = ConnectionState::Error;
          }
        }
      },
      ConnectionState::Connected => {},
      ConnectionState::Closing(_) => {},
    };
  }

  #[doc(hidden)]
  pub fn handle_content_header_frame(&mut self, channel_id: u16, size: u64, properties: BasicProperties) {
    let state = self.channels.get_mut(&channel_id).map(|channel| {
      channel.state.clone()
    }).unwrap();
    if let ChannelState::WillReceiveContent(queue_name, consumer_tag) = state {
      if size > 0 {
        self.set_channel_state(channel_id, ChannelState::ReceivingContent(queue_name.clone(), consumer_tag.clone(), size as usize));
      } else {
        self.set_channel_state(channel_id, ChannelState::Connected);
      }
      if let Some(ref mut c) = self.channels.get_mut(&channel_id) {
        if let Some(ref mut q) = c.queues.get_mut(&queue_name) {
          if let Some(ref consumer_tag) = consumer_tag {
            if let Some(ref mut cs) = q.consumers.get_mut(consumer_tag) {
              cs.set_delivery_properties(properties);
              if size == 0 {
                cs.new_delivery_complete();
              }
            }
          } else {
            q.set_delivery_properties(properties);
            if size == 0 {
              q.new_delivery_complete();
            }
          }
        }
      }
    } else {
      self.set_channel_state(channel_id, ChannelState::Error);
    }
  }

  #[doc(hidden)]
  pub fn handle_body_frame(&mut self, channel_id: u16, payload: Vec<u8>) {
    let state = self.channels.get_mut(&channel_id).map(|channel| {
      channel.state.clone()
    }).unwrap();

    let payload_size = payload.len();

    if let ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size) = state {
      if remaining_size >= payload_size {
        if let Some(ref mut c) = self.channels.get_mut(&channel_id) {
          if let Some(ref mut q) = c.queues.get_mut(&queue_name) {
            if let Some(ref consumer_tag) = opt_consumer_tag {
              if let Some(ref mut cs) = q.consumers.get_mut(consumer_tag) {
                cs.receive_delivery_content(payload);
                if remaining_size == payload_size {
                  cs.new_delivery_complete();
                }
              }
            } else {
              q.receive_delivery_content(payload);
              if remaining_size == payload_size {
                q.new_delivery_complete();
              }
            }
          }
        }

        if remaining_size == payload_size {
          self.set_channel_state(channel_id, ChannelState::Connected);
        } else {
          self.set_channel_state(channel_id, ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size - payload_size));
        }
      } else {
        error!("body frame too large");
        self.set_channel_state(channel_id, ChannelState::Error);
      }
    } else {
      self.set_channel_state(channel_id, ChannelState::Error);
    }
  }

  /// generates the content header and content frames for a payload
  ///
  /// the frames will be stored in the frame queue until they're written
  /// to the network.
  pub fn send_content_frames(&mut self, channel_id: u16, class_id: u16, slice: &[u8], properties: BasicProperties) {
    let header = AMQPContentHeader {
      class_id:       class_id,
      weight:         0,
      body_size:      slice.len() as u64,
      properties:     properties,
    };
    self.send_frame(AMQPFrame::Header(channel_id, class_id, Box::new(header)));

    //a content body frame 8 bytes of overhead
    for chunk in slice.chunks(self.configuration.frame_max as usize - 8) {
      self.send_frame(AMQPFrame::Body(channel_id, Vec::from(chunk)));
    }
  }

  #[doc(hidden)]
  pub fn send_method_frame(&mut self, channel: u16, method: AMQPClass) {
    self.send_frame(AMQPFrame::Method(channel,method));
  }

  #[doc(hidden)]
  pub fn send_frame(&mut self, frame: AMQPFrame) {
    // We always hold a reference to the receiver so it's safe to unwrap
    self.frame_sender.send(frame).unwrap();
  }
}

#[cfg(test)]
mod tests {
  use env_logger;

  use super::*;
  use crate::consumer::ConsumerSubscriber;
  use crate::message::Delivery;
  use amq_protocol::protocol::basic;

  #[derive(Clone,Debug,PartialEq)]
  struct DummySubscriber;

  impl ConsumerSubscriber for DummySubscriber {
    fn new_delivery(&mut self, _delivery: Delivery) {}
    fn drop_prefetched_messages(&mut self) {}
    fn cancel(&mut self) {}
  }

  #[test]
  fn basic_consume_small_payload() {
    let _ = env_logger::try_init();

    use crate::consumer::Consumer;
    use crate::queue::Queue;

    // Bootstrap connection state to a consuming state
    let mut conn = Connection::new();
    conn.state = ConnectionState::Connected;
    conn.configuration.channel_max = 2047;
    let channel_id = conn.create_channel().unwrap();
    conn.set_channel_state(channel_id, ChannelState::Connected);
    let queue_name = "consumed".to_string();
    let mut queue = Queue::new(queue_name.clone(), 0, 0);
    let consumer_tag = "consumer-tag".to_string();
    let consumer = Consumer::new(consumer_tag.clone(), false, false, false, false, Box::new(DummySubscriber));
    queue.consumers.insert(consumer_tag.clone(), consumer);
    conn.channels.get_mut(&channel_id).map(|c| {
      c.queues.insert(queue_name.clone(), queue);
    });
    // Now test the state machine behaviour
    {
      let deliver_frame = AMQPFrame::Method(
        channel_id,
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
      let channel_state = conn.channels.get_mut(&channel_id)
        .map(|channel| channel.state.clone())
        .unwrap();
      let expected_state = ChannelState::WillReceiveContent(
        queue_name.clone(),
        Some(consumer_tag.clone())
      );
      assert_eq!(channel_state, expected_state);
    }
    {
      let header_frame = AMQPFrame::Header(
        channel_id,
        60,
        Box::new(AMQPContentHeader {
          class_id: 60,
          weight: 0,
          body_size: 2,
          properties: BasicProperties::default(),
        })
      );
      conn.handle_frame(header_frame).unwrap();
      let channel_state = conn.channels.get_mut(&channel_id)
        .map(|channel| channel.state.clone())
        .unwrap();
      let expected_state = ChannelState::ReceivingContent(queue_name.clone(), Some(consumer_tag.clone()), 2);
      assert_eq!(channel_state, expected_state);
    }
    {
      let body_frame = AMQPFrame::Body(channel_id, "{}".as_bytes().to_vec());
      conn.handle_frame(body_frame).unwrap();
      let channel_state = conn.channels.get_mut(&channel_id)
        .map(|channel| channel.state.clone())
        .unwrap();
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
    conn.state = ConnectionState::Connected;
    conn.configuration.channel_max = 2047;
    let channel_id = conn.create_channel().unwrap();
    conn.set_channel_state(channel_id, ChannelState::Connected);
    let queue_name = "consumed".to_string();
    let mut queue = Queue::new(queue_name.clone(), 0, 0);
    let consumer_tag = "consumer-tag".to_string();
    let consumer = Consumer::new(consumer_tag.clone(), false, false, false, false, Box::new(DummySubscriber));
    queue.consumers.insert(consumer_tag.clone(), consumer);
    conn.channels.get_mut(&channel_id).map(|c| {
      c.queues.insert(queue_name.clone(), queue);
    });
    // Now test the state machine behaviour
    {
      let deliver_frame = AMQPFrame::Method(
        channel_id,
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
      let channel_state = conn.channels.get_mut(&channel_id)
        .map(|channel| channel.state.clone())
        .unwrap();
      let expected_state = ChannelState::WillReceiveContent(
        queue_name.clone(),
        Some(consumer_tag.clone())
      );
      assert_eq!(channel_state, expected_state);
    }
    {
      let header_frame = AMQPFrame::Header(
        channel_id,
        60,
        Box::new(AMQPContentHeader {
          class_id: 60,
          weight: 0,
          body_size: 0,
          properties: BasicProperties::default(),
        })
      );
      conn.handle_frame(header_frame).unwrap();
      let channel_state = conn.channels.get_mut(&channel_id)
        .map(|channel| channel.state.clone())
        .unwrap();
      let expected_state = ChannelState::Connected;
      assert_eq!(channel_state, expected_state);
    }
  }
}
