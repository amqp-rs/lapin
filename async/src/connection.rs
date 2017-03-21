use std::io::{Error,ErrorKind,Result};
use std::{result,str};
use std::collections::{HashSet,HashMap,VecDeque};
use amq_protocol_types::types::*;
use amq_protocol_types::value::*;
use nom::{IResult,Offset};
use sasl::{ChannelBinding, Credentials, Secret, Mechanism};
use sasl::mechanisms::Plain;
use cookie_factory::GenError;

use format::frame::*;
use format::content::*;
use channel::Channel;
use queue::Message;
use api::{Answer,ChannelState,RequestId};
use generated::*;
use error;

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

#[derive(Clone,Debug,PartialEq)]
pub struct Configuration {
  channel_max: u16,
  frame_max:   u32,
  heartbeat:   u16,
}

#[derive(Clone,Debug,PartialEq)]
pub struct Connection {
  /// current state of the connection. In normal use it should always be ConnectionState::Connected
  pub state:            ConnectionState,
  pub channels:         HashMap<u16, Channel>,
  pub configuration:    Configuration,
  pub channel_index:    u16,
  pub prefetch_size:    u32,
  pub prefetch_count:   u16,
  /// list of message to send
  pub frame_queue:      VecDeque<Frame>,
  /// next request id
  pub request_index:    RequestId,
  /// list of finished requests
  pub finished_reqs:    HashSet<RequestId>,
}

impl Connection {
  /// creates a `Connection` object in initial state
  pub fn new() -> Connection {
    let mut h = HashMap::new();
    h.insert(0, Channel::global());

    let configuration = Configuration {
      channel_max: 0,
      frame_max:   8192,
      heartbeat:   60,
    };

    Connection {
      state:          ConnectionState::Initial,
      channels:       h,
      configuration:  configuration,
      channel_index:  1,
      prefetch_size:  0,
      prefetch_count: 0,
      frame_queue:    VecDeque::new(),
      request_index:  0,
      finished_reqs:  HashSet::new(),
    }
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
  pub fn check_state(&self, channel_id: u16, state: ChannelState) -> Option<bool> {
    self.channels
          .get(&channel_id)
          .map(|c| c.state == state)
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
  fn check_next_answer(&self, channel_id: u16, answer: Answer) -> bool {
    self.channels
          .get(&channel_id)
          .map(|c| c.awaiting.front() == Some(&answer)).unwrap_or(false)
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

  /// verifies if the request identified with the `RequestId` is finished
  ///
  /// this method can only be called once per request id, as it will be
  /// removed from the list afterwards
  pub fn is_finished(&mut self, id: RequestId) -> bool {
    self.finished_reqs.remove(&id)
  }

  /// gets the next message corresponding to a channel, queue and consumer tag
  ///
  /// if the channel id, queue and consumer tag have no link, the method
  /// will return None. If there is no message, the method will return None
  pub fn next_message(&mut self, channel_id: u16, queue_name: &str, consumer_tag: &str) -> Option<Message> {
    self.channels.get_mut(&channel_id)
      .and_then(|channel| channel.queues.get_mut(queue_name))
      .and_then(|queue| queue.next_message(consumer_tag))
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
        gen_content_header_frame((send_buffer, 0), channel_id, class_id, header.body_size).map(|tup| tup.1)
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

    self.handle_frame(f);

    return Ok((consumed, self.state));
  }

  /// updates the current state with a new received frame
  pub fn handle_frame(&mut self, f: Frame) {
    match f {
      Frame::ProtocolHeader => {
        error!("error: the client should not receive a protocol header");
        self.state = ConnectionState::Error;
      },
      Frame::Method(channel_id, method) => {
        if channel_id == 0 {
          self.handle_global_method(method);
        } else {
          self.receive_method(channel_id, method);
        }
      },
      Frame::Heartbeat(_) => {
        self.frame_queue.push_back(Frame::Heartbeat(0));
      },
      Frame::Header(channel_id, _, header) => {
        self.handle_content_header_frame(channel_id, header.body_size);
      },
      Frame::Body(channel_id, payload) => {
        self.handle_body_frame(channel_id, payload);
      }
    };
  }

  #[doc(hidden)]
  pub fn handle_global_method(&mut self, c: Class) {
    match self.state {
      ConnectionState::Initial | ConnectionState::Closed | ConnectionState::Error => {
        self.state = ConnectionState::Error
      },
      ConnectionState::Connecting(connecting_state) => {
        match connecting_state {
          ConnectingState::Initial | ConnectingState::Error => {
            self.state = ConnectionState::Error
          },
          ConnectingState::SentProtocolHeader => {
            if let Class::Connection(connection::Methods::Start(s)) = c {
              trace!("Server sent Connection::Start: {:?}", s);
              self.state = ConnectionState::Connecting(ConnectingState::ReceivedStart);

              let mut h = FieldTable::new();
              h.insert("product".to_string(), AMQPValue::LongString("lapin".to_string()));

              let creds = Credentials {
                username: Some("guest".to_owned()),
                secret: Secret::Password("guest".to_owned()),
                channel_binding: ChannelBinding::None,
              };

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
                  response:  s.to_string(),     //FIXME: implement SASL
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
              self.configuration.heartbeat   = t.heartbeat;

              if t.frame_max > self.configuration.frame_max {
                //FIXME: what do we do without the buffers available?
                //self.send_buffer.grow(t.frame_max as usize);
                //self.receive_buffer.grow(t.frame_max as usize);
              }

              self.configuration.frame_max = t.frame_max;

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
                    virtual_host: "/".to_string(),
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
  pub fn handle_content_header_frame(&mut self, channel_id: u16, size: u64) {
    let state = self.channels.get_mut(&channel_id).map(|channel| {
      channel.state.clone()
    }).unwrap();
    if let ChannelState::WillReceiveContent(queue_name, consumer_tag) = state {
      self.set_channel_state(channel_id, ChannelState::ReceivingContent(queue_name, consumer_tag.clone(), size as usize));
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

    if let ChannelState::ReceivingContent(queue_name, consumer_tag, remaining_size) = state {
      if remaining_size >= payload_size {

        if let Some(ref mut c) = self.channels.get_mut(&channel_id) {
          if let Some(ref mut q) = c.queues.get_mut(&queue_name) {
            if let Some(ref mut cs) = q.consumers.get_mut(&consumer_tag) {
              cs.current_message.as_mut().map(|msg| msg.receive_content(payload));
              if remaining_size == payload_size {
                let message = cs.current_message.take().expect("there should be an in flight message in the consumer");
                cs.messages.push_back(message);
              }
            }
          }
        }

        if remaining_size == payload_size {
          self.set_channel_state(channel_id, ChannelState::Connected);
        } else {
          self.set_channel_state(channel_id, ChannelState::ReceivingContent(queue_name, consumer_tag, remaining_size - payload_size));
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
  /// WARNING: this does not support chunking the message yet
  ///
  /// the frames will be stored in the frame queue until they're written
  /// to the network.
  pub fn send_content_frames(&mut self, channel_id: u16, class_id: u16, slice: &[u8]) {
    let header = ContentHeader {
      class_id:       class_id,
      weight:         0,
      body_size:      slice.len() as u64,
      property_flags: 0x2000,
      property_list:  FieldTable::new(),
    };
    self.frame_queue.push_back(Frame::Header(channel_id, class_id, header));
    self.frame_queue.push_back(Frame::Body(channel_id,Vec::from(slice)));
  }

  #[doc(hidden)]
  pub fn send_method_frame(&mut self, channel: u16, method: Class) -> result::Result<(), error::Error> {
    self.frame_queue.push_back(Frame::Method(channel,method));
    Ok(())
  }
}
