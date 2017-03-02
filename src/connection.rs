use std::io::{Error,ErrorKind,Result,Write};
use std::iter::repeat;

use format::frame::gen_protocol_header;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ConnectionState {
  Initial,
  Connected,
  Error,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Connection {
  pub state:    ConnectionState,
  pub buffer:   Vec<u8>,
  pub position: usize,
  pub end:      usize,
}

impl Connection {
  pub fn new() -> Connection {
    let mut v: Vec<u8> = Vec::with_capacity(4096);
    v.extend(repeat(0).take(4096));
    Connection {
      state:    ConnectionState::Initial,
      buffer:   v,
      position: 0,
      end:      0,
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
}

