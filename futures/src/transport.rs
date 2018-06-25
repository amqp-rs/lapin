/// low level wrapper for the state machine, encoding and decoding from lapin-async
use lapin_async::connection::*;
use amq_protocol::frame::{AMQPFrame, gen_frame, parse_frame};

use nom::Offset;
use cookie_factory::GenError;
use bytes::{BufMut, BytesMut};
use std::cmp;
use std::iter::repeat;
use std::io::{self,Error,ErrorKind};
use futures::{Async,AsyncSink,Poll,Sink,StartSend,Stream,Future,future};
use tokio_codec::{Decoder,Encoder,Framed};
use tokio_io::{AsyncRead,AsyncWrite};
use channel::BasicProperties;
use client::ConnectionOptions;

/// During my testing, it appeared to be the "best" value.
/// To fine-tune it, use a queue with a very large amount of messages, consume it and compare the
/// delivery rate with the ack one.
/// Decreasing this number will increase the ack rate slightly, but decrease the delivery rate by a
/// lot.
/// Increasing this number will increate the delivery rate slightly, but the ack rate will be very
/// close to 0.
const POLL_RECV_LIMIT: u32 = 128;

/// implements tokio-io's Decoder and Encoder
pub struct AMQPCodec {
    pub frame_max: u32,
}

impl Decoder for AMQPCodec {
    type Item = AMQPFrame;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<AMQPFrame>, io::Error> {
        let (consumed, f) = match parse_frame(buf) {
          Err(e) => {
            if e.is_incomplete() {
              return Ok(None);
            } else {
              return Err(io::Error::new(io::ErrorKind::Other, format!("parse error: {:?}", e)));
            }
          },
          Ok((i, frame)) => {
            (buf.offset(i), frame)
          }
        };

        trace!("amqp decoder; frame={:?}", f);

        buf.split_to(consumed);

        Ok(Some(f))
    }
}

impl Encoder for AMQPCodec {
    type Item = AMQPFrame;
    type Error = io::Error;

    fn encode(&mut self, frame: AMQPFrame, buf: &mut BytesMut) -> Result<(), Self::Error> {
      let frame_max = cmp::max(self.frame_max, 8192) as usize;
      trace!("encoder; frame={:?}", frame);
      let offset = buf.len();
      loop {
        // If the buffer starts running out of capacity (the threshold is 1/4 of a frame), we
        // reserve more bytes upfront to avoid putting too much strain on the allocator.
        if buf.remaining_mut() < frame_max / 4 {
          trace!("encoder; reserve={}", frame_max * 2);
          buf.reserve(frame_max * 2);
        }

        let gen_res = gen_frame((buf, offset), &frame).map(|tup| tup.1);

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
  upstream:  Framed<T,AMQPCodec>,
  pub conn:  Connection,
  heartbeat: Option<AMQPFrame>,
}

impl<T> AMQPTransport<T>
   where T: AsyncRead+AsyncWrite,
         T: Send,
         T: 'static               {

  /// starts the connection process
  ///
  /// returns a future of a `AMQPTransport` that is connected
  pub fn connect(stream: T, options: ConnectionOptions) -> impl Future<Item = AMQPTransport<T>, Error = io::Error> + Send + 'static {
    let mut conn = Connection::new();
    conn.set_credentials(&options.username, &options.password);
    conn.set_vhost(&options.vhost);
    conn.set_frame_max(options.frame_max);
    conn.set_heartbeat(options.heartbeat);

    future::result(conn.connect()).map_err(|e| {
      let err = format!("Failed to connect: {:?}", e);
      Error::new(ErrorKind::ConnectionAborted, err)
    }).and_then(|_| {
        let codec = AMQPCodec {
          frame_max: conn.configuration.frame_max,
        };
        let t = AMQPTransport {
          upstream:  codec.framed(stream),
          conn:      conn,
          heartbeat: Some(AMQPFrame::Heartbeat(0)),
        };

        AMQPTransportConnector {
          transport: Some(t),
        }
    })
  }

  /// Send a frame to the broker.
  ///
  /// # Notes
  ///
  /// This function only appends the frame to a queue, to actually send the frame you have to
  /// call either `poll` or `poll_send`.
  pub fn send_frame(&mut self, frame: AMQPFrame) {
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

  /// Preemptively send an heartbeat frame
  pub fn send_heartbeat(&mut self) -> Poll<(), io::Error> {
    if let Some(frame) = self.heartbeat.take() {
      self.conn.frame_queue.push_front(frame);
    }
    self.poll_send().map(|r| r.map(|r| {
      // poll_send succeeded, reinitialize self.heartbeat so that we sned a new frame on the next
      // send_heartbeat call
      self.heartbeat = Some(AMQPFrame::Heartbeat(0));
      r
    }))
  }

  /// Poll the network to receive & handle incoming frames.
  ///
  /// # Return value
  ///
  /// This function will always return `Ok(Async::NotReady)` except in two cases:
  ///
  /// * In case of error, it will return `Err(e)`
  /// * If the socket was closed, it will return `Ok(Async::Ready(()))`
  fn poll_recv(&mut self) -> Poll<(), io::Error> {
    for _ in 0..POLL_RECV_LIMIT {
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
    Ok(Async::NotReady)
  }

  /// Poll the network to send outcoming frames.
  fn poll_send(&mut self) -> Poll<(), io::Error> {
    let mut ret = Ok(Async::Ready(()));
    while let Some(frame) = self.conn.next_frame() {
      trace!("transport poll_send; frame={:?}", frame);
      match self.start_send(frame)? {
        AsyncSink::Ready => {
          trace!("transport poll_send; status=Ready");
        },
        AsyncSink::NotReady(frame) => {
          trace!("transport poll_send; status=NotReady");
          self.conn.frame_queue.push_front(frame);
          ret = Ok(Async::NotReady);
          break;
        }
      }
    }
    if self.poll_complete()?.is_ready() {
      ret
    } else {
      Ok(Async::NotReady)
    }
  }
}

impl<T> Stream for AMQPTransport<T>
    where T: AsyncRead + AsyncWrite,
          T: Send,
          T: 'static {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<()>, io::Error> {
      trace!("transport poll");
      if let Async::Ready(()) = self.poll_recv()? {
        trace!("poll transport; status=Ready");
        return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "The connection was closed by the remote peer"));
      }
      self.poll_send().map(|r| r.map(Some))
    }
}

impl <T> Sink for AMQPTransport<T>
    where T: AsyncWrite,
          T: Send,
          T: 'static {
    type SinkItem = AMQPFrame;
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

    transport.poll()?;

    trace!("connector poll; state=ConnectionState::{:?}", transport.conn.state);
    if transport.conn.state == ConnectionState::Connected {
      return Ok(Async::Ready(transport))
    }

    self.transport = Some(transport);
    Ok(Async::NotReady)
  }
}

#[macro_export]
macro_rules! lock_transport (
    ($t: expr) => ({
        match $t.lock() {
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
  extern crate env_logger;

  use super::*;

  #[test]
  fn encode_multiple_frames() {
    let _ = env_logger::try_init();

    let mut codec = AMQPCodec { frame_max: 8192 };
    let mut buffer = BytesMut::with_capacity(8192);
    let r = codec.encode(AMQPFrame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(8, buffer.len());
    let r = codec.encode(AMQPFrame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(16, buffer.len());
    let r = codec.encode(AMQPFrame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(24, buffer.len());
  }

  #[test]
  fn encode_nested_frame() {
    use amq_protocol::frame::AMQPContentHeader;

    let _ = env_logger::try_init();

    let mut codec = AMQPCodec { frame_max: 8192 };
    let mut buffer = BytesMut::with_capacity(8192);
    let frame = AMQPFrame::Header(0, 10, AMQPContentHeader {
      class_id: 10,
      weight: 0,
      body_size: 64,
      properties: BasicProperties::default()
    });
    let r = codec.encode(frame, &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(22, buffer.len());
  }

  #[test]
  fn encode_initial_extend_buffer() {
    let _ = env_logger::try_init();

    let mut codec = AMQPCodec { frame_max: 8192 };
    let frame_max = codec.frame_max as usize;
    let mut buffer = BytesMut::new();

    let r = codec.encode(AMQPFrame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(true, buffer.capacity() >= frame_max);
    assert_eq!(8, buffer.len());
  }

  #[test]
  fn encode_anticipation_extend_buffer() {
    let _ = env_logger::try_init();

    let mut codec = AMQPCodec { frame_max: 8192 };
    let frame_max = codec.frame_max as usize;
    let mut buffer = BytesMut::new();

    let r = codec.encode(AMQPFrame::Heartbeat(0), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(frame_max * 2, buffer.capacity());
    assert_eq!(8, buffer.len());

    let payload = repeat(0u8)
      // Use 80% of the remaining space (it shouldn't trigger buffer capacity expansion)
      .take(((buffer.capacity() as f64 - buffer.len() as f64) * 0.8) as usize)
      .collect::<Vec<u8>>();
    let r = codec.encode(AMQPFrame::Body(1, payload), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(frame_max * 2, buffer.capacity());

    let payload = repeat(0u8)
      // Use 80% of the remaining space (it should trigger a buffer capacity expansion)
      .take(((buffer.capacity() as f64 - buffer.len() as f64) * 0.8) as usize)
      .collect::<Vec<u8>>();
    let r = codec.encode(AMQPFrame::Body(1, payload), &mut buffer);
    assert_eq!(false, r.is_err());
    assert_eq!(frame_max * 4, buffer.capacity());
  }
}
