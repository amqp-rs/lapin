use std::io::{Error,ErrorKind,Read,Result,Write};
use std::iter::repeat;
use std::collections::HashMap;
use nom::{HexDisplay,IResult,Offset};

use format::frame::{frame,Frame,FrameType,gen_protocol_header,raw_frame};
use channel::Channel;
use generated::*;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ConnectionState {
  Initial,
  Connected,
  Error,
}

#[derive(Clone,Debug,PartialEq)]
pub struct Connection {
  pub state:    ConnectionState,
  pub channels: HashMap<u16, Channel>,
  pub buffer:   Vec<u8>,
  pub receive_buffer: Vec<u8>,
  pub position: usize,
  pub end:      usize,
  pub receive_position: usize,
  pub receive_end: usize,
}

impl Connection {
  pub fn new() -> Connection {
    let mut v: Vec<u8> = Vec::with_capacity(8192);
    v.extend(repeat(0).take(8192));
    let mut v2: Vec<u8> = Vec::with_capacity(8192);
    v2.extend(repeat(0).take(8192));

    let mut h = HashMap::new();
    h.insert(0, Channel::global());

    Connection {
      state:    ConnectionState::Initial,
      channels: h,
      buffer:   v,
      receive_buffer: v2,
      position: 0,
      end:      0,
      receive_position: 0,
      receive_end: 0,
    }
  }

  pub fn connect(&mut self, writer: &mut Write) -> Result<ConnectionState> {
    if self.state != ConnectionState::Initial {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    let res = gen_protocol_header((&mut self.buffer[self.end..], 0)).map(|tup| tup.1);
    if let Ok(sz) = res {
      self.end += sz;
      match writer.write(&mut self.buffer[..self.end]) {
        Ok(sz) => {
          self.position += sz;
          self.state = ConnectionState::Connected;
          Ok(self.state)
        },
        Err(e) => Err(e),
      }
    } else {
      Err(Error::new(ErrorKind::WouldBlock, "could not write protocol header"))
    }
  }

  pub fn read(&mut self, reader: &mut Read) -> Result<ConnectionState> {
    if self.state != ConnectionState::Connected {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    match reader.read(&mut self.receive_buffer[self.receive_end..]) {
      Ok(sz) => {
        self.receive_end += sz;
        println!("will parse:\n{}", (&self.receive_buffer[self.receive_position..self.receive_end]).to_hex(16));
        match raw_frame(&self.receive_buffer[self.receive_position..self.receive_end]) {
          IResult::Done(i, f) => {
            println!("parsed frame: {:?}", f);
            self.receive_position = self.receive_buffer.offset(i);
            match f.frame_type {
              FrameType::Method => {
                println!("parsed method: {:?}", parse_class(f.payload));
              },
              t => {
                println!("frame type: {:?} -> unknown payload:\n{}", t, f.payload.to_hex(16));
              }
            }
            Ok(ConnectionState::Connected)
          },
          e => {
            let err = format!("parse error: {:?}", e);
            return Err(Error::new(ErrorKind::Other, err))
          }
        }
      },
      Err(e) => Err(e),
    }
  }
}

