use connection::{Connection,ConnectionState};
use buffer::Buffer;

use std::io::{Error,ErrorKind,Read,Result,Write};

impl Connection {
  /// helper function to handle reading and writing repeatedly from the network until there's no more state to update
  pub fn run<T>(&mut self, stream: &mut T, send_buffer: &mut Buffer, receive_buffer: &mut Buffer) -> Result<ConnectionState>
    where T: Read + Write {

    let mut write_would_block = false;
    let mut read_would_block  = false;

    loop {
      let continue_writing = !write_would_block && self.can_write(send_buffer);
      let continue_reading = !read_would_block && self.can_read(receive_buffer);
      let continue_parsing = self.can_parse(receive_buffer);

      if !continue_writing && !continue_reading && !continue_parsing {
        return Ok(self.state);
      }

      if continue_writing {
        match self.write_to_stream(stream, send_buffer) {
          Ok((_,_)) => {

          },
          Err(e) => {
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
      }

      if continue_reading {
        match self.read_from_stream(stream, receive_buffer) {
          Ok(_) => {},
          Err(e) => {
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
  pub fn can_write(&self, send_buffer: &Buffer) -> bool {
    send_buffer.available_data() > 0 || !self.frame_queue.is_empty()
  }

  /// tests whether we can read from the receive buffer
  pub fn can_read(&self, receive_buffer: &Buffer) -> bool {
    receive_buffer.available_space() > 0
  }

  /// tests whether we can parse data from the receive buffer
  pub fn can_parse(&self, receive_buffer: &Buffer) -> bool {
    receive_buffer.available_data() > 0
  }

  /// serializes frames to the send buffer then to the writer (if possible)
  pub fn write_to_stream(&mut self, writer: &mut Write, send_buffer: &mut Buffer) -> Result<(usize, ConnectionState)> {
    match self.serialize(send_buffer.space()) {
      Ok((sz, _)) => {
        send_buffer.fill(sz);
      },
      Err(e) => {
        return Err(e);
      }
    }

    match writer.write(&mut send_buffer.data()) {
      Ok(sz) => {
        trace!("wrote {} bytes", sz);
        send_buffer.consume(sz);
        Ok((sz, self.state))
      },
      Err(e) => Err(e),
    }
  }

  /// read data from the network into the receive buffer
  pub fn read_from_stream(&mut self, reader: &mut Read, receive_buffer: &mut Buffer) -> Result<(usize, ConnectionState)> {
    if self.state == ConnectionState::Initial || self.state == ConnectionState::Error {
      self.state = ConnectionState::Error;
      return Err(Error::new(ErrorKind::Other, "invalid state"))
    }

    match reader.read(&mut receive_buffer.space()) {
      Ok(sz) => {
        trace!("read {} bytes", sz);
        receive_buffer.fill(sz);
        Ok((sz, self.state))
      },
      Err(e) => Err(e),
    }
  }
}
