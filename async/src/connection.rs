use std::{result,str};
use std::default::Default;
use std::io::{Error,ErrorKind,Result};
use std::collections::{HashMap,VecDeque};
use nom::{IResult,Offset};
use sasl;
use sasl::client::Mechanism;
use sasl::client::mechanisms::Plain;
use cookie_factory::GenError;

use format::frame::*;
use format::content::*;
use channel::Channel;
use message::*;
use api::{Answer,ChannelState,RequestId};
use generated::*;
use types::{AMQPValue,FieldTable};
use error::{self, InvalidState};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ConnectionState {
  Initial,
  Connecting(ConnectingState),
  Connected,
  Closing(ClosingState),
  Closed,
  Error,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ConnectingState {
  Initial,
  SentProtocolHeader,
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

#[derive(Clone,Debug,Default,PartialEq)]
pub struct Configuration {
  channel_max:   u16,
  pub frame_max: u32,
  pub heartbeat: u16,
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

#[derive(Clone,Debug,PartialEq)]
pub struct Connection {
  /// current state of the connection. In normal use it should always be ConnectionState::Connected
  pub state:             ConnectionState,
  pub channels:          HashMap<u16, Channel>,
  pub configuration:     Configuration,
  pub vhost:             String,
  pub channel_index:     u16,
  pub prefetch_size:     u32,
  pub prefetch_count:    u16,
  /// list of message to send
  pub frame_queue:       VecDeque<Frame>,
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

    let configuration = Configuration::default();

    Connection {
      state:             ConnectionState::Initial,
      channels:          h,
      configuration:     configuration,
      vhost:             "/".to_string(),
      channel_index:     1,
      prefetch_size:     0,
      prefetch_count:    0,
      frame_queue:       VecDeque::new(),
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

  pub fn set_frame_max(&mut self, frame_max: u32) {
    self.configuration.frame_max = frame_max;
  }

  /// creates a `Channel` object in initial state
  ///
  /// returns a `u16` channel id
  ///
  /// The channel will not be usable until `channel_open`
  /// is called with the channel id
  pub fn create_channel(&mut self) -> u16 {
    let c  = Channel::new(self.channel_index);
    self.channels.insert(self.channel_index, c);
    self.channel_index += 1;

    self.channel_index - 1
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
          .map_or(Err(error::Error::InvalidChannel), |c| {
              if c.state == state {
                  Ok(())
              } else {
                Err(error::Error::InvalidState(InvalidState {
                    expected: state,
                    actual:   c.state.clone(),
                }))
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

  /// gets the next message corresponding to a channel, queue and consumer tag
  ///
  /// if the channel id, queue and consumer tag have no link, the method
  /// will return None. If there is no message, the method will return None
  pub fn next_delivery(&mut self, channel_id: u16, queue_name: &str, consumer_tag: &str) -> Option<Delivery> {
    self.channels.get_mut(&channel_id)
      .and_then(|channel| channel.queues.get_mut(queue_name))
      .and_then(|queue| queue.next_delivery(consumer_tag))
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
  pub fn connect(&mut self) -> Result<ConnectionState> {
    if self.state != ConnectionState::Initial {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    self.frame_queue.push_back(Frame::ProtocolHeader);
    self.state = ConnectionState::Connecting(ConnectingState::SentProtocolHeader);
    Ok(self.state)
  }

  /// next message to send to the network
  ///
  /// returns None if there's no message to send
  pub fn next_frame(&mut self) -> Option<Frame> {
    self.frame_queue.pop_front()
  }

  /// writes the next message to a mutable byte slice
  ///
  /// returns how many bytes were written and the current state.
  /// this method can be called repeatedly until the buffer is full or
  /// there are no more frames to send
  pub fn serialize(&mut self, send_buffer: &mut [u8]) -> Result<(usize, ConnectionState)> {
    let next_msg = self.frame_queue.pop_front();
    if next_msg == None {
      return Err(Error::new(ErrorKind::WouldBlock, "no new message"));
    }

    let next_msg = next_msg.unwrap();
    trace!("will write to buffer: {:?}", next_msg);

    let gen_res = match &next_msg {
      &Frame::ProtocolHeader => {
        gen_protocol_header((send_buffer, 0)).map(|tup| tup.1)
      },
      &Frame::Heartbeat(_) => {
        gen_heartbeat_frame((send_buffer, 0)).map(|tup| tup.1)
      },
      &Frame::Method(channel, ref method) => {
        gen_method_frame((send_buffer, 0), channel, method).map(|tup| tup.1)
      },
      &Frame::Header(channel_id, class_id, ref header) => {
        gen_content_header_frame((send_buffer, 0), channel_id, class_id, header.body_size, &header.properties).map(|tup| tup.1)
      },
      &Frame::Body(channel_id, ref data) => {
        gen_content_body_frame((send_buffer, 0), channel_id, data).map(|tup| tup.1)
      }
    };

    match gen_res {
      Ok(sz) => {
        Ok((sz, self.state))
      },
      Err(e) => {
        error!("error generating frame: {:?}", e);
        self.state = ConnectionState::Error;
        match e {
          GenError::BufferTooSmall(_) => {
            self.frame_queue.push_front(next_msg);
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
    let parsed_frame = frame(data);
    match parsed_frame {
      IResult::Done(_,_)     => {},
      IResult::Incomplete(_) => {
        return Ok((0,self.state));
      },
      IResult::Error(e) => {
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

    return Ok((consumed, self.state));
  }

  /// updates the current state with a new received frame
  pub fn handle_frame(&mut self, f: Frame) -> result::Result<(), error::Error> {
    trace!("will handle frame: {:?}", f);
    match f {
      Frame::ProtocolHeader => {
        error!("error: the client should not receive a protocol header");
        self.state = ConnectionState::Error;
      },
      Frame::Method(channel_id, method) => {
        if channel_id == 0 {
          self.handle_global_method(method);
        } else {
          self.receive_method(channel_id, method)?;
        }
      },
      Frame::Heartbeat(_) => {
        debug!("received heartbeat from server");
      },
      Frame::Header(channel_id, _, header) => {
        self.handle_content_header_frame(channel_id, header.body_size, header.properties);
      },
      Frame::Body(channel_id, payload) => {
        self.handle_body_frame(channel_id, payload);
      }
    };
    Ok(())
  }

  #[doc(hidden)]
  pub fn handle_global_method(&mut self, c: Class) {
    match self.state {
      ConnectionState::Initial | ConnectionState::Closed | ConnectionState::Error => {
        self.state = ConnectionState::Error
      },
      ConnectionState::Connecting(connecting_state) => {
        match connecting_state {
          ConnectingState::Initial => {
            self.state = ConnectionState::Error
          },
          ConnectingState::SentProtocolHeader => {
            if let Class::Connection(connection::Methods::Start(s)) = c {
              trace!("Server sent Connection::Start: {:?}", s);
              self.state = ConnectionState::Connecting(ConnectingState::ReceivedStart);

              let mut h = FieldTable::new();
              h.insert("product".to_string(), AMQPValue::LongString("lapin".to_string()));

              let saved_creds = self.credentials.take().unwrap_or(Credentials::default());

              let creds = sasl::common::Credentials::default()
                .with_username(saved_creds.username)
                .with_password(saved_creds.password);

              let mut mechanism = Plain::from_credentials(creds).unwrap();

              let initial_data = mechanism.initial().unwrap();
              let s = str::from_utf8(&initial_data).unwrap();

              //FIXME: fill with user configured data
              //we need to handle the server properties, and have some client properties
              let start_ok = Class::Connection(connection::Methods::StartOk(
                connection::StartOk {
                  client_properties: h,
                  mechanism: "PLAIN".to_string(),
                  locale:    "en_US".to_string(), // FIXME: comes from the server
                  response:  s.to_string(),
                }
              ));

              debug!("client sending Connection::StartOk: {:?}", start_ok);
              self.frame_queue.push_back(Frame::Method(0, start_ok));
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
            if let Class::Connection(connection::Methods::Tune(t)) = c {
              debug!("Server sent Connection::Tune: {:?}", t);
              self.state = ConnectionState::Connecting(ConnectingState::ReceivedTune);

              self.configuration.channel_max = t.channel_max;
              if self.configuration.heartbeat == 0 {
                // If we disable the heartbeat but the server don't, follow him and enable it too
                self.configuration.heartbeat = t.heartbeat;
              } else if t.heartbeat != 0 && t.heartbeat < self.configuration.heartbeat {
                // If both us and the server want heartbeat enabled, pick the lowest value.
                self.configuration.heartbeat = t.heartbeat;
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

              let tune_ok = Class::Connection(connection::Methods::TuneOk(
                connection::TuneOk {
                  channel_max : self.configuration.channel_max,
                  frame_max   : self.configuration.frame_max,
                  heartbeat   : self.configuration.heartbeat,
                }
              ));

              debug!("client sending Connection::TuneOk: {:?}", tune_ok);

              self.frame_queue.push_back(Frame::Method(0, tune_ok));
              self.state = ConnectionState::Connecting(ConnectingState::SentTuneOk);

              let open = Class::Connection(connection::Methods::Open(
                  connection::Open {
                    virtual_host: self.vhost.clone(),
                    capabilities: "".to_string(),
                    insist:       false,
                  }
                  ));

              debug!("client sending Connection::Open: {:?}", open);
              self.frame_queue.push_back(Frame::Method(0,open));
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
            if let Class::Connection(connection::Methods::OpenOk(o)) = c {
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
  pub fn handle_content_header_frame(&mut self, channel_id: u16, size: u64, properties: basic::Properties) {
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
              if let Some(msg) = cs.current_message.as_mut() {
                msg.properties = properties;
              }
              if size == 0 {
                let message = cs.current_message.take().expect("there should be an in flight message in the consumer");
                cs.messages.push_back(message);
              }
            }
          } else {
            if let Some(msg) = q.current_get_message.as_mut() {
              msg.delivery.properties = properties;
            }
            if size == 0 {
              let message = q.current_get_message.take().expect("there should be an in flight message in the queue");
              q.get_messages.push_back(message);
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
                cs.current_message.as_mut().map(|msg| msg.receive_content(payload));
                if remaining_size == payload_size {
                  let message = cs.current_message.take().expect("there should be an in flight message in the consumer");
                  cs.messages.push_back(message);
                }
              }
            } else {
              q.current_get_message.as_mut().map(|msg| msg.delivery.receive_content(payload));
              if remaining_size == payload_size {
                let message = q.current_get_message.take().expect("there should be an in flight message in the queue");
                q.get_messages.push_back(message);
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
  pub fn send_content_frames(&mut self, channel_id: u16, class_id: u16, slice: &[u8], properties: basic::Properties) {
    let header = ContentHeader {
      class_id:       class_id,
      weight:         0,
      body_size:      slice.len() as u64,
      properties:     properties,
    };
    self.frame_queue.push_back(Frame::Header(channel_id, class_id, header));

    //a content body frame 8 bytes of overhead
    for chunk in slice.chunks(self.configuration.frame_max as usize - 8) {
      self.frame_queue.push_back(Frame::Body(channel_id, Vec::from(chunk)));
    }
  }

  #[doc(hidden)]
  pub fn send_method_frame(&mut self, channel: u16, method: Class) -> result::Result<(), error::Error> {
    self.frame_queue.push_back(Frame::Method(channel,method));
    Ok(())
  }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_consume_small_payload() {
        use queue::{Consumer, Queue};

        // Bootstrap connection state to a consuming state
        let mut conn = Connection::new();
        conn.state = ConnectionState::Connected;
        let channel_id = conn.create_channel();
        conn.set_channel_state(channel_id, ChannelState::Connected);
        let queue_name = "consumed".to_string();
        let mut queue = Queue::new(queue_name.clone(), 0, 0);
        let consumer_tag = "consumer-tag".to_string();
        let consumer = Consumer {
            tag: consumer_tag.clone(),
            no_local: false,
            no_ack: false,
            exclusive: false,
            nowait: false,
            current_message: None,
            messages: VecDeque::new(),
        };
        queue.consumers.insert(consumer_tag.clone(), consumer);
        conn.channels.get_mut(&channel_id).map(|c| {
            c.queues.insert(queue_name.clone(), queue);
        });
        // Now test the state machine behaviour
        {
            let deliver_frame = Frame::Method(
                channel_id,
                Class::Basic(
                    basic::Methods::Deliver(
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
            let header_frame = Frame::Header(
                channel_id,
                60,
                ContentHeader {
                    class_id: 60,
                    weight: 0,
                    body_size: 2,
                    properties: basic::Properties::default(),
                }
            );
            conn.handle_frame(header_frame).unwrap();
            let channel_state = conn.channels.get_mut(&channel_id)
                .map(|channel| channel.state.clone())
                .unwrap();
            let expected_state = ChannelState::ReceivingContent(queue_name.clone(), Some(consumer_tag.clone()), 2);
            assert_eq!(channel_state, expected_state);
        }
        {
           let body_frame = Frame::Body(channel_id, "{}".as_bytes().to_vec());
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
        use queue::{Consumer, Queue};

        // Bootstrap connection state to a consuming state
        let mut conn = Connection::new();
        conn.state = ConnectionState::Connected;
        let channel_id = conn.create_channel();
        conn.set_channel_state(channel_id, ChannelState::Connected);
        let queue_name = "consumed".to_string();
        let mut queue = Queue::new(queue_name.clone(), 0, 0);
        let consumer_tag = "consumer-tag".to_string();
        let consumer = Consumer {
            tag: consumer_tag.clone(),
            no_local: false,
            no_ack: false,
            exclusive: false,
            nowait: false,
            current_message: None,
            messages: VecDeque::new(),
        };
        queue.consumers.insert(consumer_tag.clone(), consumer);
        conn.channels.get_mut(&channel_id).map(|c| {
            c.queues.insert(queue_name.clone(), queue);
        });
        // Now test the state machine behaviour
        {
            let deliver_frame = Frame::Method(
                channel_id,
                Class::Basic(
                    basic::Methods::Deliver(
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
            let header_frame = Frame::Header(
                channel_id,
                60,
                ContentHeader {
                    class_id: 60,
                    weight: 0,
                    body_size: 0,
                    properties: basic::Properties::default(),
                }
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
