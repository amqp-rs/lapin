use log::{error, trace};

use std::io::{Error, ErrorKind, Read, Result, Write};

use crate::buffer::Buffer;
use crate::connection::{Connection, ConnectionState};

impl Connection {
  /// helper function to handle reading and writing repeatedly from the network until there's no more state to update
  pub fn run<T: Read + Write>(&mut self, stream: &mut T, send_buffer: &mut Buffer, receive_buffer: &mut Buffer) -> Result<ConnectionState> {
    let mut write_would_block = false;
    let mut read_would_block  = false;

    loop {
      let continue_writing = !write_would_block && self.can_write(send_buffer);
      let continue_reading = !read_would_block && self.can_read(receive_buffer);
      let continue_parsing = self.can_parse(receive_buffer);

      if !continue_writing && !continue_reading && !continue_parsing {
        return Ok(self.state.clone());
      }

      if continue_writing {
        if let Err(e) = self.write_to_stream(stream, send_buffer) {
          match e.kind() {
            ErrorKind::WouldBlock => {
              write_would_block = true;
            },
            k => {
              error!("error writing: {:?}", k);
              self.state = ConnectionState::Error;
              return Err(e);
            }
          }
        }
      }

      if continue_reading {
        if let Err(e) = self.read_from_stream(stream, receive_buffer) {
          match e.kind() {
            ErrorKind::WouldBlock => {
              read_would_block = true;
            },
            k => {
              error!("error reading: {:?}", k);
              self.state = ConnectionState::Error;
              return Err(e);
            }
          }
        }
      }

      if continue_parsing {
        //FIXME: handle the Incomplete case. We need a WantRead and WantWrite signal
        if let Ok((sz, _)) = self.parse(receive_buffer.data()) {
          receive_buffer.consume(sz);
        }
      }
    }
  }

  /// tests whether we can write to the send buffer
  fn can_write(&self, send_buffer: &Buffer) -> bool {
    send_buffer.available_data() > 0 || !self.frame_receiver.is_empty()
  }

  /// tests whether we can read from the receive buffer
  fn can_read(&self, receive_buffer: &Buffer) -> bool {
    receive_buffer.available_space() > 0
  }

  /// tests whether we can parse data from the receive buffer
  fn can_parse(&self, receive_buffer: &Buffer) -> bool {
    receive_buffer.available_data() > 0
  }

  /// serializes frames to the send buffer then to the writer (if possible)
  fn write_to_stream(&mut self, writer: &mut dyn Write, send_buffer: &mut Buffer) -> Result<(usize, ConnectionState)> {
    let (sz, _) = self.serialize(send_buffer.space())?;
    send_buffer.fill(sz);

    writer.write(&mut send_buffer.data()).map(|sz| {
      trace!("wrote {} bytes", sz);
      send_buffer.consume(sz);
      (sz, self.state.clone())
    })
  }

  /// read data from the network into the receive buffer
  fn read_from_stream(&mut self, reader: &mut dyn Read, receive_buffer: &mut Buffer) -> Result<(usize, ConnectionState)> {
    if self.state == ConnectionState::Initial || self.state == ConnectionState::Error {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"));
    }

    reader.read(&mut receive_buffer.space()).map(|sz| {
      trace!("read {} bytes", sz);
      receive_buffer.fill(sz);
      (sz, self.state.clone())
    })
  }
}
