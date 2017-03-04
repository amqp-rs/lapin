use std::io::{Error,ErrorKind,Read,Result,Write};
use std::iter::repeat;
use std::collections::HashMap;
use nom::{HexDisplay,IResult,Offset};

use format::frame::{frame,Frame,FrameType,gen_protocol_header,raw_frame};
use channel::Channel;
use buffer::Buffer;
use generated::*;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ConnectionState {
  Initial,
  Connecting,
  Connected,
  Error,
}

#[derive(Clone,Debug,PartialEq)]
pub struct Connection {
  pub state:            ConnectionState,
  pub channels:         HashMap<u16, Channel>,
  pub send_buffer:      Buffer,
  pub receive_buffer:   Buffer,
}

impl Connection {
  pub fn new() -> Connection {
    let mut h = HashMap::new();
    h.insert(0, Channel::global());

    Connection {
      state:    ConnectionState::Initial,
      channels: h,
      send_buffer:    Buffer::with_capacity(8192),
      receive_buffer: Buffer::with_capacity(8192),
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
          self.state = ConnectionState::Connecting;
          Ok(self.state)
        },
        Err(e) => Err(e),
      }
    } else {
      Err(Error::new(ErrorKind::WouldBlock, "could not write protocol header"))
    }
  }

  pub fn read(&mut self, reader: &mut Read) -> Result<ConnectionState> {
    if self.state == ConnectionState::Initial || self.state == ConnectionState::Error {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    match reader.read(&mut self.receive_buffer.space()) {
      Ok(sz) => {
        self.receive_buffer.fill(sz);
      },
      Err(e) => return Err(e),
    }
    println!("will parse:\n{}", (&self.receive_buffer.data()).to_hex(16));
    let (channel_id, method, consumed) = {
      let parsed_frame = raw_frame(&self.receive_buffer.data());
      match parsed_frame {
        IResult::Done(i,_) => {}
        e => {
          //FIXME: should probably disconnect on error here
          let err = format!("parse error: {:?}", e);
          return Err(Error::new(ErrorKind::Other, err))
        }
      }

      let (i, f) = parsed_frame.unwrap();

      println!("parsed frame: {:?}", f);
      //FIXME: what happens if we fail to parse a packet in a channel?
      // do we continue?
      let consumed = self.receive_buffer.data().offset(i);

      match f.frame_type {
        FrameType::Method => {
          let parsed = parse_class(f.payload);
          println!("parsed method: {:?}", parsed);
          match parsed {
            IResult::Done(b"", m) => {
              (f.channel_id, m, consumed)
            },
            e => {
              //FIXME: should probably disconnect channel on error here
              let err = format!("parse error: {:?}", e);
              return Err(Error::new(ErrorKind::Other, err))
            }
          }
        },
        t => {
          println!("frame type: {:?} -> unknown payload:\n{}", t, f.payload.to_hex(16));
          let err = format!("parse error: {:?}", t);
          return Err(Error::new(ErrorKind::Other, err))
        }
      }
    };

    self.receive_buffer.consume(consumed);

    if channel_id == 0 {
      self.handle_global_method(method);
    }

    return Ok(ConnectionState::Connected);


    unreachable!();
  }

  pub fn handle_global_method(&mut self, c: Class) {
    if let Class::Connection(method) = c {
      println!("handling connection: {:?}", method);
    }

  }
}

