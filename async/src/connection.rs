use std::io::{Error,ErrorKind,Read,Result,Write};
use std::{result,str};
use std::iter::repeat;
use std::collections::{HashMap,VecDeque};
use amq_protocol_types::AMQPValue;
use nom::{HexDisplay,IResult,Offset};
use sasl::{ChannelBinding, Credentials, Secret, Mechanism};
use sasl::mechanisms::Plain;
use cookie_factory::GenError;

use format::frame::*;
use format::field::*;
use format::content::*;
use channel::{Channel,LocalFrame};
use api::{Answer,ChannelState};
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
pub struct Connection<'a> {
  pub state:            ConnectionState,
  pub channels:         HashMap<u16, Channel<'a>>,
  pub send_buffer:      Buffer,
  pub receive_buffer:   Buffer,
  pub configuration:    Configuration,
  pub channel_index:    u16,
  pub prefetch_size:    u32,
  pub prefetch_count:   u16,
  pub frame_queue:      VecDeque<LocalFrame>,
}

impl<'a> Connection<'a> {
  pub fn new() -> Connection<'a> {
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
      send_buffer:    Buffer::with_capacity(configuration.frame_max as usize),
      receive_buffer: Buffer::with_capacity(configuration.frame_max as usize),
      configuration:  configuration,
      channel_index:  1,
      prefetch_size:  0,
      prefetch_count: 0,
      frame_queue:    VecDeque::new(),
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

  pub fn connect(&mut self, writer: &mut Write) -> Result<ConnectionState> {
    if self.state != ConnectionState::Initial {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    let res = gen_protocol_header((self.send_buffer.space(), 0)).map(|tup| tup.1);
    if let Ok(sz) = res {
      self.send_buffer.fill(sz);
      match writer.write(&mut self.send_buffer.data()) {
        Ok(sz2) => {
          self.send_buffer.consume(sz2);
          self.state = ConnectionState::Connecting(ConnectingState::SentProtocolHeader);
          Ok(self.state)
        },
        Err(e) => Err(e),
      }
    } else {
      Err(Error::new(ErrorKind::WouldBlock, "could not write protocol header"))
    }
  }

  pub fn run<T>(&mut self, stream: &mut T) -> Result<ConnectionState>
    where T: Read + Write {

    let mut write_would_block = false;
    let mut read_would_block  = false;

    loop {
      let continue_writing = !write_would_block && self.can_write();
      let continue_reading = !read_would_block && self.can_read();
      let continue_parsing = self.can_parse();

      if !continue_writing && !continue_reading && !continue_parsing {
        return Ok(self.state);
      }

      if continue_writing {
        match self.write(stream) {
          Ok((sz,_)) => {

          },
          Err(e) => {
            match e.kind() {
              ErrorKind::WouldBlock => {
                write_would_block = true;
              },
              k => {
                println!("error writing: {:?}", k);
                self.state = ConnectionState::Error;
                return Err(e);
              }
            }
          }
        }
      }

      if continue_reading {
        match self.read(stream) {
          Ok(_) => {},
          Err(e) => {
            match e.kind() {
              ErrorKind::WouldBlock => {
                read_would_block = true;
              },
              k => {
                println!("error reading: {:?}", k);
                self.state = ConnectionState::Error;
                return Err(e);
              }
            }
          }
        }
      }

      if continue_parsing {
        //FIXME: handle the Incomplete case. We need a WantRead and WantWrite signal
        self.parse();
      }
    }

    let res:Result<ConnectionState> = Ok(self.state);
    res
  }

  pub fn can_write(&self) -> bool {
    self.send_buffer.available_data() > 0 || !self.frame_queue.is_empty()
  }

  pub fn can_read(&self) -> bool {
    self.receive_buffer.available_space() > 0
  }

  pub fn can_parse(&self) -> bool {
    self.receive_buffer.available_data() > 0
  }

  pub fn write(&mut self, writer: &mut Write) -> Result<(usize, ConnectionState)> {
    let next_msg = self.frame_queue.pop_front();
    if next_msg == None {
      return Err(Error::new(ErrorKind::WouldBlock, "no new message"));
    }

    let next_msg = next_msg.unwrap();
    println!("will write to buffer: {:?}", next_msg);

    let gen_res = match &next_msg {
      &LocalFrame::HeartBeat => {
        gen_heartbeat_frame((&mut self.send_buffer.space(), 0)).map(|tup| tup.1)
      },
      &LocalFrame::Method(channel, ref method) => {
        gen_method_frame((&mut self.send_buffer.space(), 0), channel, method).map(|tup| tup.1)
      },
      &LocalFrame::Header(channel_id, class_id, size) => {
        gen_content_header_frame((&mut self.send_buffer.space(), 0), channel_id, class_id, size).map(|tup| tup.1)
      },
      &LocalFrame::Body(channel_id, ref data) => {
        gen_content_body_frame((&mut self.send_buffer.space(), 0), channel_id, data).map(|tup| tup.1)
      }
    };

    match gen_res {
      Ok(sz) => {
        self.send_buffer.fill(sz);
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

    //println!("will write:\n{}", (&self.send_buffer.data()).to_hex(16));
    match writer.write(&mut self.send_buffer.data()) {
      Ok(sz) => {
        println!("wrote {} bytes", sz);
        self.send_buffer.consume(sz);
        Ok((sz, self.state))
      },
      Err(e) => Err(e),
    }
  }

  pub fn read(&mut self, reader: &mut Read) -> Result<(usize, ConnectionState)> {
    if self.state == ConnectionState::Initial || self.state == ConnectionState::Error {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    match reader.read(&mut self.receive_buffer.space()) {
      Ok(sz) => {
        println!("read {} bytes", sz);
        self.receive_buffer.fill(sz);
        Ok((sz, self.state))
      },
      Err(e) => Err(e),
    }
  }

  pub fn parse(&mut self) -> Result<(usize,ConnectionState)> {
    //println!("will parse:\n{}", (&self.receive_buffer.data()).to_hex(16));
    let (channel_id, method, consumed) = {
      let parsed_frame = raw_frame(&self.receive_buffer.data());
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

      //println!("parsed frame: {:?}", f);
      //FIXME: what happens if we fail to parse a packet in a channel?
      // do we continue?
      let consumed = self.receive_buffer.data().offset(i);

      let method = match f.frame_type {
        FrameType::Method => {
          let parsed = parse_class(f.payload);
          println!("parsed method: {:?}", parsed);
          match parsed {
            IResult::Done(b"", m) => {
              Some(m)
            },
            e => {
              //we should not get an incomplete here
              //FIXME: should probably disconnect channel on error here
              let err = format!("parse error: {:?}", e);
              if f.channel_id == 0 {
                self.state = ConnectionState::Error;
              } else {
                self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::Error);
              }
              return Err(Error::new(ErrorKind::Other, err))
            }
          }
        },
        FrameType::Heartbeat => {
          self.frame_queue.push_back(LocalFrame::HeartBeat);

          None
        },
        FrameType::Header => {
          let parsed = content_header(f.payload);
          println!("parsed method: {:?}", parsed);
          match parsed {
            IResult::Done(b"", m) => {
              let state = self.channels.get_mut(&f.channel_id).map(|channel| {
                channel.state.clone()
              }).unwrap();
              if let ChannelState::WillReceiveContent(consumer_tag) = state {
                self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::ReceivingContent(consumer_tag.clone(), m.body_size as usize));
              } else {
                self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::Error);
              }
              None
            },
            e => {
              //we should not get an incomplete here
              //FIXME: should probably disconnect channel on error here
              let err = format!("parse error: {:?}", e);
              if f.channel_id == 0 {
                self.state = ConnectionState::Error;
              } else {
                self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::Error);
              }
              return Err(Error::new(ErrorKind::Other, err));
            }
          }
        }
        FrameType::Body => {
          let state = self.channels.get_mut(&f.channel_id).map(|channel| {
            channel.state.clone()
          }).unwrap();

          if let ChannelState::ReceivingContent(consumer_tag, remaining_size) = state {
            if remaining_size >= f.payload.len() {

              self.channels.get_mut(&f.channel_id).map(|c| {
                for (_, ref mut q) in &mut c.queues {
                  q.consumers.get_mut(&consumer_tag).map(| cs| {
                    (*cs.callback).receive_content(f.payload);
                      if remaining_size == f.payload.len() {
                        (*cs.callback).done();
                      }
                  });
                }
              });

              if remaining_size == f.payload.len() {
                self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::Connected);
              } else {
                self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::ReceivingContent(consumer_tag, remaining_size - f.payload.len()));
              }
            } else {
              println!("body frame too large");
              self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::Error);
            }
          } else {
            self.channels.get_mut(&f.channel_id).map(|channel| channel.state = ChannelState::Error);
          }
          None
        }
        t => {
          println!("frame type: {:?} -> unknown payload:\n{}", t, f.payload.to_hex(16));
          let err = format!("parse error: {:?}", t);
          return Err(Error::new(ErrorKind::Other, err))
        }
      };

      (f.channel_id, method, consumed)
    };

    self.receive_buffer.consume(consumed);

    if let Some(method) = method {
      if channel_id == 0 {
        self.handle_global_method(method);
      } else {
        self.receive_method(channel_id, method);
      }
    }


    return Ok((consumed, self.state));
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
              self.frame_queue.push_back(LocalFrame::Method(0, start_ok));
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
                self.send_buffer.grow(t.frame_max as usize);
                self.receive_buffer.grow(t.frame_max as usize);
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

              self.frame_queue.push_back(LocalFrame::Method(0, tune_ok));
              self.state = ConnectionState::Connecting(ConnectingState::SentTuneOk);

              let open = Class::Connection(connection::Methods::Open(
                  connection::Open {
                    virtual_host: "/".to_string(),
                    capabilities: "".to_string(),
                    insist:       false,
                  }
                  ));

              println!("client sending Connection::Open: {:?}", open);
              self.frame_queue.push_back(LocalFrame::Method(0,open));
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

  pub fn handle_body_frame(&mut self, channel_id: u16, payload: &[u8]) {
    let state = self.channels.get_mut(&channel_id).map(|channel| {
      channel.state.clone()
    }).unwrap();

    if let ChannelState::ReceivingContent(consumer_tag, remaining_size) = state {
      if remaining_size >= payload.len() {

        self.channels.get_mut(&channel_id).map(|c| {
          for (_, ref mut q) in &mut c.queues {
            q.consumers.get_mut(&consumer_tag).map(| cs| {
              (*cs.callback).receive_content(payload);
              if remaining_size == payload.len() {
                (*cs.callback).done();
              }
            });
          }
        });

        if remaining_size == payload.len() {
          self.channels.get_mut(&channel_id).map(|channel| channel.state = ChannelState::Connected);
        } else {
          self.channels.get_mut(&channel_id).map(|channel| channel.state = ChannelState::ReceivingContent(consumer_tag, remaining_size - payload.len()));
        }
      } else {
        println!("body frame too large");
        self.channels.get_mut(&channel_id).map(|channel| channel.state = ChannelState::Error);
      }
    } else {
      self.channels.get_mut(&channel_id).map(|channel| channel.state = ChannelState::Error);
    }
  }

  //FIXME: no chunking for now
  pub fn send_content_frames(&mut self, channel_id: u16, class_id: u16, slice: &[u8]) {
    self.frame_queue.push_back(LocalFrame::Header(channel_id, class_id, slice.len() as u64));
    self.frame_queue.push_back(LocalFrame::Body(channel_id,Vec::from(slice)));
  }

  pub fn send_method_frame(&mut self, channel: u16, method: Class) -> result::Result<(), error::Error> {
    self.frame_queue.push_back(LocalFrame::Method(channel,method));
    Ok(())
  }
}
