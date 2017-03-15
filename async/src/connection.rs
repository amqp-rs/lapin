use std::io::{Error,ErrorKind,Read,Result,Write};
use std::{result,str};
use std::iter::repeat;
use std::collections::{HashSet,HashMap,VecDeque};
use amq_protocol_types::AMQPValue;
use nom::{HexDisplay,IResult,Offset};
use sasl::{ChannelBinding, Credentials, Secret, Mechanism};
use sasl::mechanisms::Plain;
use cookie_factory::GenError;

use format::frame::*;
use format::field::*;
use format::content::*;
use channel::Channel;
use queue::Message;
use api::{Answer,ChannelState,RequestId};
use buffer::Buffer;
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
  pub state:            ConnectionState,
  pub channels:         HashMap<u16, Channel>,
  pub configuration:    Configuration,
  pub channel_index:    u16,
  pub prefetch_size:    u32,
  pub prefetch_count:   u16,
  pub frame_queue:      VecDeque<Frame>,
  pub request_index:    RequestId,
  pub finished_reqs:    HashSet<RequestId>,
}

impl Connection {
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

  pub fn create_channel(&mut self) -> u16 {
    let c  = Channel::new(self.channel_index);
    self.channels.insert(self.channel_index, c);
    self.channel_index += 1;

    self.channel_index - 1
  }

  pub fn set_channel_state(&mut self, channel_id: u16, new_state: ChannelState) {
    self.channels.get_mut(&channel_id).map(|c| c.state = new_state);
  }

  pub fn check_state(&self, channel_id: u16, state: ChannelState) -> Option<bool> {
    self.channels
          .get(&channel_id)
          .map(|c| c.state == state)
  }

  pub fn get_state(&self, channel_id: u16) -> Option<ChannelState> {
    self.channels
          .get(&channel_id)
          .map(|c| c.state.clone())
  }

  pub fn push_back_answer(&mut self, channel_id: u16, answer: Answer) {
    self.channels
      .get_mut(&channel_id)
      .map(|c| c.awaiting.push_back(answer));
  }

  pub fn check_next_answer(&self, channel_id: u16, answer: Answer) -> bool {
    self.channels
          .get(&channel_id)
          .map(|c| c.awaiting.front() == Some(&answer)).unwrap_or(false)
  }

  pub fn get_next_answer(&mut self, channel_id: u16) -> Option<Answer> {
    self.channels
          .get_mut(&channel_id)
          .and_then(|c| c.awaiting.pop_front())
  }

  pub fn is_connected(&self, channel_id: u16) -> bool {
    self.channels
          .get(&channel_id)
          .map(|c| c.is_connected()).unwrap_or(false)
  }

  pub fn next_request_id(&mut self) -> RequestId {
    let id = self.request_index;
    self.request_index += 1;
    id
  }

  pub fn is_finished(&mut self, id: RequestId) -> bool {
    self.finished_reqs.remove(&id)
  }

  pub fn next_message(&mut self, channel_id: u16, queue_name: &str, consumer_tag: &str) -> Option<Message> {
    self.channels.get_mut(&channel_id)
      .and_then(|channel| channel.queues.get_mut(queue_name))
      .and_then(|queue| queue.next_message(consumer_tag))
  }

  pub fn connect(&mut self) -> Result<ConnectionState> {
    if self.state != ConnectionState::Initial {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    self.frame_queue.push_back(Frame::ProtocolHeader);
    self.state = ConnectionState::Connecting(ConnectingState::SentProtocolHeader);
    Ok(self.state)
  }

  pub fn next_frame(&mut self) -> Option<Frame> {
    self.frame_queue.pop_front()
  }

  pub fn serialize(&mut self, send_buffer: &mut [u8]) -> Result<(usize, ConnectionState)> {
    let next_msg = self.frame_queue.pop_front();
    if next_msg == None {
      return Err(Error::new(ErrorKind::WouldBlock, "no new message"));
    }

    let next_msg = next_msg.unwrap();
    println!("will write to buffer: {:?}", next_msg);

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
        println!("error generating frame: {:?}", e);
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

  pub fn parse(&mut self, data: &[u8]) -> Result<(usize,ConnectionState)> {
    let parsed_frame = frame(data);
    match parsed_frame {
      IResult::Done(i,_)     => {},
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

  pub fn handle_frame(&mut self, f: Frame) {
    match f {
      Frame::ProtocolHeader => {
        println!("error: the client should not receive a protocol header");
        self.state = ConnectionState::Error;
      },
      Frame::Method(channel_id, method) => {
        if channel_id == 0 {
          self.handle_global_method(method);
        } else {
          self.receive_method(channel_id, method);
        }
      },
      Frame::Heartbeat(channel_id) => {
        self.frame_queue.push_back(Frame::Heartbeat(0));
      },
      Frame::Header(channel_id, class_id, header) => {
        self.handle_content_header_frame(channel_id, header.body_size);
      },
      Frame::Body(channel_id, payload) => {
        self.handle_body_frame(channel_id, payload);
      }
    };
  }

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
              println!("Server sent Connection::Start: {:?}", s);
              self.state = ConnectionState::Connecting(ConnectingState::ReceivedStart);

              let mut h = HashMap::new();
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

              println!("client sending Connection::StartOk: {:?}", start_ok);
              self.frame_queue.push_back(Frame::Method(0, start_ok));
              self.state = ConnectionState::Connecting(ConnectingState::SentStartOk);
            } else {
              println!("waiting for class Connection method Start, got {:?}", c);
              self.state = ConnectionState::Error;
            }
          },
          /*ConnectingState::ReceivedStart => {
            println!("state {:?}\treceived\t{:?}", self.state, c);
          },*/
          ConnectingState::SentStartOk => {
            if let Class::Connection(connection::Methods::Tune(t)) = c {
              println!("Server sent Connection::Tune: {:?}", t);
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

              println!("client sending Connection::TuneOk: {:?}", tune_ok);

              self.frame_queue.push_back(Frame::Method(0, tune_ok));
              self.state = ConnectionState::Connecting(ConnectingState::SentTuneOk);

              let open = Class::Connection(connection::Methods::Open(
                  connection::Open {
                    virtual_host: "/".to_string(),
                    capabilities: "".to_string(),
                    insist:       false,
                  }
                  ));

              println!("client sending Connection::Open: {:?}", open);
              self.frame_queue.push_back(Frame::Method(0,open));
              self.state = ConnectionState::Connecting(ConnectingState::SentOpen);

            } else {
              println!("waiting for class Connection method Start, got {:?}", c);
              self.state = ConnectionState::Error;
            }
          },
          ConnectingState::ReceivedSecure => {
            println!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::SentSecure => {
            println!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::ReceivedSecondSecure => {
            println!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::ReceivedTune => {
            println!("state {:?}\treceived\t{:?}", self.state, c);
          },
          ConnectingState::SentOpen => {
            println!("state {:?}\treceived\t{:?}", self.state, c);
            if let Class::Connection(connection::Methods::OpenOk(o)) = c {
              println!("Server sent Connection::OpenOk: {:?}, client now connected", o);
              self.state = ConnectionState::Connected;
            } else {
              println!("waiting for class Connection method Start, got {:?}", c);
              self.state = ConnectionState::Error;
            }
          },
          ConnectingState::Error => {
            println!("state {:?}\treceived\t{:?}", self.state, c);
          },
          s => {
            println!("invalid state {:?}", s);
            self.state = ConnectionState::Error;
          }
        }
      },
      ConnectionState::Connected => {},
      ConnectionState::Closing(ClosingState) => {},
    };
  }

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
        println!("body frame too large");
        self.set_channel_state(channel_id, ChannelState::Error);
      }
    } else {
      self.set_channel_state(channel_id, ChannelState::Error);
    }
  }

  //FIXME: no chunking for now
  pub fn send_content_frames(&mut self, channel_id: u16, class_id: u16, slice: &[u8]) {
    let header = ContentHeader {
      class_id:       class_id,
      weight:         0,
      body_size:      slice.len() as u64,
      property_flags: 0x2000,
      property_list:  HashMap::new(),
    };
    self.frame_queue.push_back(Frame::Header(channel_id, class_id, header));
    self.frame_queue.push_back(Frame::Body(channel_id,Vec::from(slice)));
  }

  pub fn send_method_frame(&mut self, channel: u16, method: Class) -> result::Result<(), error::Error> {
    self.frame_queue.push_back(Frame::Method(channel,method));
    Ok(())
  }
}
