use std::io::{Error,ErrorKind,Read,Result,Write};
use std::str;
use std::iter::repeat;
use std::collections::HashMap;
use amq_protocol_types::AMQPValue;
use nom::{HexDisplay,IResult,Offset};
use sasl::{ChannelBinding, Credentials, Secret, Mechanism};
use sasl::mechanisms::Plain;

use format::frame::*;
use format::field::*;
use channel::{Channel,ChannelState};
use buffer::Buffer;
use generated::*;

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
  pub send_buffer:      Buffer,
  pub receive_buffer:   Buffer,
  pub configuration:    Configuration,
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
      send_buffer:    Buffer::with_capacity(configuration.frame_max as usize),
      receive_buffer: Buffer::with_capacity(configuration.frame_max as usize),
      configuration:  configuration,
    }
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

  pub fn write(&mut self, writer: &mut Write) -> Result<ConnectionState> {
    println!("will write:\n{}", (&self.send_buffer.data()).to_hex(16));
    match writer.write(&mut self.send_buffer.data()) {
      Ok(sz) => {
        println!("wrote {} bytes", sz);
        self.send_buffer.consume(sz);
        Ok(self.state)
      },
      Err(e) => Err(e),
    }
  }

  pub fn read(&mut self, reader: &mut Read) -> Result<ConnectionState> {
    if self.state == ConnectionState::Initial || self.state == ConnectionState::Error {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    match reader.read(&mut self.receive_buffer.space()) {
      Ok(sz) => {
        println!("read {} bytes", sz);
        self.receive_buffer.fill(sz);
      },
      Err(e) => return Err(e),
    }
    println!("will parse:\n{}", (&self.receive_buffer.data()).to_hex(16));
    let (channel_id, method, consumed) = {
      let parsed_frame = raw_frame(&self.receive_buffer.data());
      match parsed_frame {
        IResult::Done(i,_)     => {},
        IResult::Incomplete(_) => {
          return Ok(self.state);
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

      match f.frame_type {
        FrameType::Method => {
          let parsed = parse_class(f.payload);
          println!("parsed method: {:?}", parsed);
          match parsed {
            IResult::Done(b"", m) => {
              (f.channel_id, Some(m), consumed)
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
          match gen_heartbeat_frame((&mut self.send_buffer.space(), 0)).map(|tup| tup.1) {
            Ok(sz) => {
              self.send_buffer.fill(sz);
            }
            Err(e) => {
              println!("error generating start-ok frame: {:?}", e);
              self.state = ConnectionState::Error;
            },
          };

          (f.channel_id, None, consumed)
        },
        t => {
          println!("frame type: {:?} -> unknown payload:\n{}", t, f.payload.to_hex(16));
          let err = format!("parse error: {:?}", t);
          return Err(Error::new(ErrorKind::Other, err))
        }
      }
    };

    self.receive_buffer.consume(consumed);

    if let Some(method) = method {
      if channel_id == 0 {
        self.handle_global_method(method);
      } else {
        self.channels.get_mut(&channel_id).map(|channel| channel.received_method(method));
      }
    }


    return Ok(self.state);


    unreachable!();
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
              let start_ok = Class::Connection(connection::Methods::StartOk(
                connection::StartOk {
                  client_properties: h,
                  mechanism: "PLAIN".to_string(),
                  locale:    "en_US".to_string(), // FIXME: comes from the server
                  response:  s.to_string(),     //FIXME: implement SASL
                }
              ));

              println!("client sending Connection::StartOk: {:?}", start_ok);

              match gen_method_frame((&mut self.send_buffer.space(), 0), 0, &start_ok).map(|tup| tup.1) {
                Ok(sz) => {
                  self.send_buffer.fill(sz);
                  self.state = ConnectionState::Connecting(ConnectingState::SentStartOk);
                }
                Err(e) => {
                  println!("error generating start-ok frame: {:?}", e);
                  self.state = ConnectionState::Error;
                },
              }
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

              match gen_method_frame((&mut self.send_buffer.space(), 0), 0, &tune_ok).map(|tup| tup.1) {
                Ok(sz) => {
                  self.send_buffer.fill(sz);
                  self.state = ConnectionState::Connecting(ConnectingState::SentTuneOk);

                  let open = Class::Connection(connection::Methods::Open(
                    connection::Open {
                      virtual_host: "/".to_string(),
                      capabilities: "".to_string(),
                      insist:       false,
                    }
                  ));

                  println!("client sending Connection::Open: {:?}", open);

                  match gen_method_frame((&mut self.send_buffer.space(), 0), 0, &open).map(|tup| tup.1) {
                    Ok(sz) => {
                      self.send_buffer.fill(sz);
                      self.state = ConnectionState::Connecting(ConnectingState::SentOpen);
                    }
                    Err(e) => {
                      println!("error generating start-ok frame: {:?}", e);
                      self.state = ConnectionState::Error;
                    },
                  }
                }
                Err(e) => {
                  println!("error generating start-ok frame: {:?}", e);
                  self.state = ConnectionState::Error;
                },
              }
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

  //FIXME: no chunking for now
  pub fn send_content_frames(&mut self, channel_id: u16, class_id: u16, slice: &[u8]) {
    match gen_content_header_frame((&mut self.send_buffer.space(), 0), channel_id, class_id, slice.len() as u64).map(|tup| tup.1) {
      Ok(sz) => {
        self.send_buffer.fill(sz);
        match gen_content_body_frame((&mut self.send_buffer.space(), 0), channel_id, slice).map(|tup| tup.1) {
          Ok(sz) => {
            self.send_buffer.fill(sz);
          },
          Err(e) => {
            println!("error generating frame: {:?}", e);
            self.state = ConnectionState::Error;
          }
        }
      },
      Err(e) => {
        println!("error generating frame: {:?}", e);
        self.state = ConnectionState::Error;
      }
    }
  }
}
