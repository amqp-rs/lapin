use amq_protocol::frame::{GenError, Offset, gen_frame, parse_frame};
use log::{error, trace};

use std::io::{self, Read, Write};

use crate::{
  buffer::Buffer,
  connection::Connection,
  connection_status::ConnectionState,
  error::{Error, ErrorKind},
};

impl Connection {
  /// helper function to handle reading and writing repeatedly from the network until there's no more state to update
  pub fn run<T: Read + Write>(&mut self, stream: &mut T, send_buffer: &mut Buffer, receive_buffer: &mut Buffer) -> Result<ConnectionState, Error> {
    let mut write_would_block = false;
    let mut read_would_block  = false;

    loop {
      let continue_writing = !write_would_block && self.can_write(send_buffer) && !self.status.blocked();
      let continue_reading = !read_would_block && self.can_read(receive_buffer);
      let continue_parsing = self.can_parse(receive_buffer);

      if !continue_writing && !continue_reading && !continue_parsing {
        return Ok(self.status.state());
      }

      if continue_writing {
        if let Err(e) = self.write_to_stream(stream, send_buffer) {
          match e.kind() {
            ErrorKind::IOError(ie) if ie.kind() == io::ErrorKind::WouldBlock => write_would_block = true,
            k => {
              error!("error writing: {:?}", k);
              self.set_error()?;
              return Err(e);
            }
          }
        }
      }

      if continue_reading {
        if let Err(e) = self.read_from_stream(stream, receive_buffer) {
          match e.kind() {
            ErrorKind::NoNewMessage => read_would_block = true,
            ErrorKind::IOError(ie) if ie.kind() == io::ErrorKind::WouldBlock => read_would_block = true,
            k => {
              error!("error reading: {:?}", k);
              self.set_error()?;
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
    send_buffer.available_data() > 0 || self.has_pending_frames()
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
  fn write_to_stream(&mut self, writer: &mut dyn Write, send_buffer: &mut Buffer) -> Result<(usize, ConnectionState), Error> {
    let (sz, _) = self.serialize(send_buffer.space())?;
    send_buffer.fill(sz);

    writer.write(&send_buffer.data()).map(|sz| {
      trace!("wrote {} bytes", sz);
      send_buffer.consume(sz);
      (sz, self.status.state())
    }).map_err(|e| ErrorKind::IOError(e).into())
  }

  /// read data from the network into the receive buffer
  fn read_from_stream(&mut self, reader: &mut dyn Read, receive_buffer: &mut Buffer) -> Result<(usize, ConnectionState), Error> {
    let state = self.status.state();
    if state == ConnectionState::Initial || state == ConnectionState::Closed || state == ConnectionState::Error {
      self.set_error()?;
      Err(ErrorKind::InvalidConnectionState(state).into())
    } else {
      reader.read(&mut receive_buffer.space()).map(|sz| {
        trace!("read {} bytes", sz);
        receive_buffer.fill(sz);
        (sz, state)
      }).map_err(|e| ErrorKind::IOError(e).into())
    }
  }

  /// writes the next message to a mutable byte slice
  ///
  /// returns how many bytes were written and the current state.
  /// this method can be called repeatedly until the buffer is full or
  /// there are no more frames to send
  fn serialize(&self, send_buffer: &mut [u8]) -> Result<(usize, ConnectionState), Error> {
    if let Some(next_msg) = self.next_frame() {
      trace!("will write to buffer: {:?}", next_msg);
      match gen_frame((send_buffer, 0), &next_msg).map(|tup| tup.1) {
        Ok(sz) => {
          Ok((sz, self.status.state()))
        },
        Err(e) => {
          error!("error generating frame: {:?}", e);
          self.set_error()?;
          match e {
            GenError::BufferTooSmall(_) => {
              // Requeue msg
              self.requeue_frame(next_msg);
              Err(ErrorKind::SendBufferTooSmall.into())
            },
            GenError::InvalidOffset | GenError::CustomError(_) | GenError::NotYetImplemented => {
              Err(ErrorKind::SerialisationError(e).into())
            }
          }
        }
      }
    } else {
      Err(ErrorKind::NoNewMessage.into())
    }
  }

  /// parses a frame from a byte slice
  ///
  /// returns how many bytes were consumed and the current state.
  ///
  /// This method will update the state machine according to the ReceivedStart
  /// frame with `handle_frame`
  fn parse(&self, data: &[u8]) -> Result<(usize,ConnectionState), Error> {
    match parse_frame(data) {
      Ok((i, f)) => {
        let consumed = data.offset(i);

        if let Err(e) = self.handle_frame(f) {
          self.set_error()?;
          Err(e)
        } else {
          Ok((consumed, self.status.state()))
        }
      },
      Err(e) => {
        if e.is_incomplete() {
          Ok((0,self.status.state()))
        } else {
          self.set_error()?;
          Err(ErrorKind::ParsingError(format!("{:?}", e)).into())
        }
      }
    }
  }

}
