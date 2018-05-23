/// low level wrapper for the state machine, encoding and decoding from lapin-async
use lapin_async::connection::*;
use lapin_async::format::frame::*;

use nom::{IResult,Offset};
use cookie_factory::GenError;
use bytes::{BufMut, BytesMut};
use std::cmp;
use std::iter::repeat;
use std::io::{self,Error,ErrorKind};
use futures::{Async,AsyncSink,Poll,Sink,StartSend,Stream,Future,future};
use tokio_io::{AsyncRead,AsyncWrite};
use tokio_io::codec::{Decoder,Encoder,Framed};
use channel::BasicProperties;
use client::ConnectionOptions;

/// implements tokio-io's Decoder and Encoder
pub struct AMQPCodec {
    pub frame_max: u32,
}

impl Decoder for AMQPCodec {
    type Item = Frame;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Frame>, io::Error> {
        let (consumed, f) = match frame(buf) {
          IResult::Incomplete(_) => {
            return Ok(None)
          },
          IResult::Error(e) => {
            return Err(io::Error::new(io::ErrorKind::Other, format!("parse error: {:?}", e)))
          },
          IResult::Done(i, frame) => {
            (buf.offset(i), frame)
          }
        };

        trace!("amqp decoder; frame={:?}", f);

        buf.split_to(consumed);

        Ok(Some(f))
    }
}

impl Encoder for AMQPCodec {
    type Item = Frame;
    type Error = io::Error;

    fn encode(&mut self, frame: Frame, buf: &mut BytesMut) -> Result<(), Self::Error> {
      // Ensure we at least allocate room for two complete frames, to make sure that forthcoming
      // `BytesMut::extend` calls do not call the allocator each time.
      let frame_max = cmp::max(self.frame_max, 8192) as usize;
      if buf.remaining_mut() < frame_max / 2 {
        trace!("encoder; reserve={}", frame_max * 2);
        buf.reserve(frame_max * 2);
      }
      trace!("encoder; frame={:?}", frame);
      let offset = buf.len();
      loop {
        let gen_res = match &frame {
          &Frame::ProtocolHeader => {
            gen_protocol_header((buf, offset)).map(|tup| tup.1)
          },
          &Frame::Heartbeat(_) => {
            gen_heartbeat_frame((buf, offset)).map(|tup| tup.1)
          },
          &Frame::Method(channel, ref method) => {
            gen_method_frame((buf, offset), channel, method).map(|tup| tup.1)
          },
          &Frame::Header(channel_id, class_id, ref header) => {
            gen_content_header_frame((buf, offset), channel_id, class_id, header.body_size, &header.properties).map(|tup| tup.1)
          },
          &Frame::Body(channel_id, ref data) => {
            gen_content_body_frame((buf, offset), channel_id, data).map(|tup| tup.1)
          }
        };

        match gen_res {
          Ok(sz) => {
            trace!("encoder; frame_size={}", sz - offset);
            return Ok(());
          },
          Err(GenError::BufferTooSmall(sz)) => {
            // BufferTooSmall error variant returns the index the next write would have
            // occured if there was enough space in the buffer. Thus we subtract the
            // buffer's length to know how much bytes we sould make available.
            let length = buf.len();
            trace!("encoder; sz={} length={} extend={}", sz, length, sz - length);
            buf.extend(repeat(0).take(sz - length));
          },
          Err(e) => {
            error!("error generating frame: {:?}", e);
            return Err(Error::new(ErrorKind::InvalidData, "could not generate"));
          }
        }
      }
    }
}

/// Wrappers over a `Framed` stream using `AMQPCodec` and lapin-async's `Connection`
pub struct AMQPTransport<T> {
  upstream: Framed<T,AMQPCodec>,
  pub conn: Connection,
}

impl<T> AMQPTransport<T>
   where T: AsyncRead+AsyncWrite,
         T: Send,
         T: 'static               {

  /// starts the connection process
  ///
  /// returns a future of a `AMQPTransport` that is connected
  pub fn connect(stream: T, options: &ConnectionOptions) -> Box<Future<Item = AMQPTransport<T>, Error = io::Error> + Send> {
    let mut conn = Connection::new();
    conn.set_credentials(&options.username, &options.password);
    conn.set_vhost(&options.vhost);
    conn.set_frame_max(options.frame_max);
    conn.set_heartbeat(options.heartbeat);
    if let Err(e) = conn.connect() {
      let err = format!("Failed to connect: {:?}", e);
      return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
    }

    let codec = AMQPCodec {
      frame_max: conn.configuration.frame_max,
    };
    let t = AMQPTransport {
      upstream:     stream.framed(codec),
      conn:         conn,
    };
    let connector = AMQPTransportConnector {
      transport: Some(t),
    };
    Box::new(connector)
  }

  /// Send a frame to the broker.
  ///
  /// # Notes
  ///
  /// This function only appends the frame to a queue, to actually send the frame you have to
  /// call either `poll` or `poll_send`.
  pub fn send_frame(&mut self, frame: Frame) {
    self.conn.frame_queue.push_back(frame);
  }

  /// Send content frames to the broker.
  ///
  /// # Notes
  ///
  /// This function only appends the frames to a queue, to actually send the frames you have to
  /// call either `poll` or `poll_send`.
  pub fn send_content_frames(&mut self, channel_id: u16, payload: &[u8], properties: BasicProperties) {
    self.conn.send_content_frames(channel_id, 60, payload, properties);
  }

  /// Poll the network to receive & handle incoming frames.
  ///
  /// # Return value
  ///
  /// This function will always return `Ok(Async::NotReady)` except in two cases:
  ///
  /// * In case of error, it will return `Err(e)`
  /// * If the socket was closed, it will return `Ok(Async::Ready(()))`
  pub fn poll_recv(&mut self) -> Poll<(), io::Error> {
    loop {
      match self.upstream.poll() {
        Ok(Async::Ready(Some(frame))) => {
          trace!("transport poll_recv; frame={:?}", frame);
          if let Err(e) = self.conn.handle_frame(frame) {
            let err = format!("failed to handle frame: {:?}", e);
            return Err(io::Error::new(io::ErrorKind::Other, err));
          }
        },
        Ok(Async::Ready(None)) => {
          trace!("transport poll_recv; status=Ready(None)");
          return Ok(Async::Ready(()));
        },
        Ok(Async::NotReady) => {
          trace!("transport poll_recv; status=NotReady");
          return Ok(Async::NotReady);
        },
        Err(e) => {
          error!("transport poll_recv; status=Err({:?})", e);
          return Err(From::from(e));
        },
      };
    }
  }

  /// Poll the network to send outcoming frames.
  pub fn poll_send(&mut self) -> Poll<(), io::Error> {
    while let Some(frame) = self.conn.next_frame() {
      trace!("transport poll_send; frame={:?}", frame);
      match self.upstream.start_send(frame)? {
        AsyncSink::Ready => {
          trace!("transport poll_send; status=Ready");
        },
        AsyncSink::NotReady(frame) => {
          trace!("transport poll_send; status=NotReady");
          self.conn.frame_queue.push_front(frame);
          return Ok(Async::NotReady);
        }
      }
    }
    self.upstream.poll_complete()
  }
}

impl<T> Future for AMQPTransport<T>
    where T: AsyncRead + AsyncWrite,
          T: Send,
          T: 'static {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        trace!("transport poll");
        if let Async::Ready(_) = self.poll_recv()? {
            return Ok(Async::Ready(()));
        }
        self.poll_send()?;
        Ok(Async::NotReady)
    }
}

impl <T> Sink for AMQPTransport<T>
    where T: AsyncWrite,
          T: Send,
          T: 'static {
    type SinkItem = Frame;
    type SinkError = io::Error;

    fn start_send(&mut self, frame: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        trace!("transport start_send; frame={:?}", frame);
        self.upstream.start_send(frame)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        trace!("transport poll_complete");
        self.upstream.poll_complete()
    }
}

/// implements a future of `AMQPTransport`
///
/// this structure is used to perform the AMQP handshake and provide
/// a connected transport afterwards
pub struct AMQPTransportConnector<T> {
  pub transport: Option<AMQPTransport<T>>,
}

impl<T> Future for AMQPTransportConnector<T>
    where T: AsyncRead + AsyncWrite,
          T: Send,
          T: 'static {

  type Item  = AMQPTransport<T>;
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    trace!("connector poll; has_transport={:?}", !self.transport.is_none());
    let mut transport = self.transport.take().unwrap();

    if let Async::Ready(_) = transport.poll()? {
      trace!("connector poll transport; status=Ready");
      return Err(io::Error::new(io::ErrorKind::Other, "The connection was closed during the handshake"));
    }

    trace!("connector poll; state=ConnectionState::{:?}", transport.conn.state);
    if transport.conn.state == ConnectionState::Connected {
      return Ok(Async::Ready(transport))
    }

    self.transport = Some(transport);
    Ok(Async::NotReady)
  }
}

#[macro_export]
macro_rules! try_lock_transport (
    ($t: expr) => ({
        match $t.try_lock() {
            Ok(t) => t,
            Err(_) => if $t.is_poisoned() {
                return Err(io::Error::new(io::ErrorKind::Other, "Transport mutex is poisoned"))
            } else {
                task::current().notify();
                return Ok(Async::NotReady)
            }
        }
    });
);

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn encode_multiple_frames() {
    let mut codec = AMQPCodec { frame_max: 8192 };
    let mut buffer = BytesMut::with_capacity(8192);
    let r = codec.encode(Frame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(8, buffer.len());
    let r = codec.encode(Frame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(16, buffer.len());
    let r = codec.encode(Frame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(24, buffer.len());
  }

  #[test]
  fn encode_extend_buffer() {
    let mut codec = AMQPCodec { frame_max: 8192 };
    let mut buffer = BytesMut::new();
    let r = codec.encode(Frame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(true, buffer.capacity() >= (codec.frame_max as usize));
    assert_ne!(0, buffer.len());
  }
}
