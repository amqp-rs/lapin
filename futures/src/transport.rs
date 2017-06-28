/// low level wrapper for the state machine, encoding and decoding from lapin-async
use lapin_async::connection::*;
use lapin_async::format::frame::*;

use nom::{IResult,Offset};
use cookie_factory::GenError;
use bytes::BytesMut;
use std::cmp;
use std::iter::repeat;
use std::io::{self,Error,ErrorKind};
use std::time::Duration;
use futures::{Async,Poll,Sink,Stream,StartSend,Future,future};
use tokio_io::{AsyncRead,AsyncWrite};
use tokio_io::codec::{Decoder,Encoder,Framed};
use tokio_timer::{Interval,Timer};
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

        trace!("decoded frame: {:?}", f);

        buf.split_to(consumed);

        Ok(Some(f))
    }
}

impl Encoder for AMQPCodec {
    type Item = Frame;
    type Error = io::Error;

    fn encode(&mut self, frame: Frame, buf: &mut BytesMut) -> Result<(), Self::Error> {
      let length    = buf.len();
      // Ensure we at least allocate 8192 so that the buffer is big enough for the frame_max
      // negociation. Afterwards, use frame_max if > 8192.
      let frame_max = cmp::max(self.frame_max, 8192) as usize;
      if length < frame_max {
        //reserve more capacity and intialize it
        buf.extend(repeat(0).take(frame_max - length));
      }
      trace!("will encode and write frame: {:?}", frame);

      loop {
        let gen_res = match &frame {
          &Frame::ProtocolHeader => {
            gen_protocol_header((buf, 0)).map(|tup| tup.1)
          },
          &Frame::Heartbeat(_) => {
            gen_heartbeat_frame((buf, 0)).map(|tup| tup.1)
          },
          &Frame::Method(channel, ref method) => {
            gen_method_frame((buf, 0), channel, method).map(|tup| tup.1)
          },
          &Frame::Header(channel_id, class_id, ref header) => {
            gen_content_header_frame((buf, 0), channel_id, class_id, header.body_size, &header.properties).map(|tup| tup.1)
          },
          &Frame::Body(channel_id, ref data) => {
            gen_content_body_frame((buf, 0), channel_id, data).map(|tup| tup.1)
          }
        };

        match gen_res {
          Ok(sz) => {
            buf.truncate(sz);
            trace!("serialized frame: {} bytes", sz);
            return Ok(());
          },
          Err(e) => {
            error!("error generating frame: {:?}", e);
            match e {
              GenError::BufferTooSmall(sz) => {
                buf.extend(repeat(0).take(sz - length));
                //return Err(Error::new(ErrorKind::InvalidData, "send buffer too small"));
              },
              GenError::InvalidOffset | GenError::CustomError(_) | GenError::NotYetImplemented => {
                return Err(Error::new(ErrorKind::InvalidData, "could not generate"));
              }
            }
          }
        }
      }
    }
}

/// Wrappers over a `Framed` stream using `AMQPCodec` and lapin-async's `Connection`
pub struct AMQPTransport<T> {
  upstream: Framed<T,AMQPCodec>,
  heartbeat: Option<Interval>,
  pub conn: Connection,
}

impl<T> AMQPTransport<T>
   where T: AsyncRead+AsyncWrite,
         T: Sync+Send,
         T: 'static               {

  /// starts the connection process
  ///
  /// returns a future of a `AMQPTransport` that is connected
  pub fn connect(stream: T, options: &ConnectionOptions) -> Box<Future<Item = AMQPTransport<T>, Error = io::Error>> {
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
    let mut t = AMQPTransport {
      upstream:  stream.framed(codec),
      heartbeat: None,
      conn:      conn,
    };

    if let Err(e) = t.send_and_handle_frames() {
      let err = format!("Failed to handle frames: {:?}", e);
      return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
    }

    let mut connector = AMQPTransportConnector {
      transport: Some(t),
    };

    trace!("pre-poll");
    if let Err(e) = connector.poll() {
      let err = format!("Failed to handle frames: {:?}", e);
      return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
    }
    trace!("post-poll");

    Box::new(connector)
  }

  pub fn start_heartbeat(&mut self) {
      let heartbeat = self.conn.configuration.heartbeat as u64;
      if heartbeat > 0 {
          self.heartbeat = Some(Timer::default().interval(Duration::from_secs(heartbeat)));
      }
  }

  fn poll_heartbeat(&mut self) -> Result<(), io::Error> {
    if let Some(Ok(Async::Ready(_))) = self.heartbeat.as_mut().map(Interval::poll) {
      debug!("Sending heartbeat");
      if let Err(e) = self.send_frame(Frame::Heartbeat(0)) {
        error!("Failed to send heartbeat: {:?}", e);
        return Err(e);
      }
    }
    Ok(())
  }

  fn poll_upstream(&mut self) -> Poll<Option<()>, io::Error> {
    trace!("poll upstream");
    let value = match self.upstream.poll() {
      Ok(Async::Ready(t)) => t,
      Ok(Async::NotReady) => {
        trace!("upstream poll gave NotReady");
        return Ok(Async::NotReady);
      },
      Err(e) => {
        error!("upstream poll gave error: {:?}", e);
        return Err(From::from(e));
      },
    };

    if let Some(frame) = value {
      trace!("upstream poll gave frame: {:?}", frame);
      if let Err(e) = self.conn.handle_frame(frame) {
        let err = format!("failed to handle frame: {:?}", e);
        return Err(io::Error::new(io::ErrorKind::Other, err));
      }
      self.send_frames()?;
      Ok(Async::Ready(Some(())))
    } else {
      error!("upstream poll gave Ready(None)");
      Ok(Async::Ready(None))
    }
  }

  pub fn send_and_handle_frames(&mut self) -> Poll<Option<()>, io::Error> {
    self.send_frames()?;
    self.handle_frames()
  }

  pub fn send_and_handle_frames_with_payload(&mut self, channel_id: u16, payload: &[u8], properties: BasicProperties) -> Poll<Option<()>, io::Error> {
      self.send_and_handle_frames()?;
      self.conn.send_content_frames(channel_id, 60, payload, properties);
      self.send_and_handle_frames()
  }

  fn send_frames(&mut self) -> Result<(), io::Error> {
    //FIXME: find a way to use a future here
    trace!("send frames");
    while let Some(f) = self.conn.next_frame() {
      if let Err(e) = self.send_frame(f) {
        error!("Failed to send frame: {:?}", e);
        return Err(e);
      }
    }
    Ok(())
  }

  fn send_frame(&mut self, frame: Frame) -> Poll<(), io::Error> {
      self.start_send(frame).and_then(|_| self.poll_complete())
  }

  fn handle_frames(&mut self) -> Poll<Option<()>, io::Error> {
    trace!("handle frames");
    for _ in 0..30 {
      if try_ready!(self.poll()).is_none() {
        return Ok(Async::Ready(None));
      }
    }
    self.poll()
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
          T: Sync+Send,
          T: 'static {

  type Item  = AMQPTransport<T>;
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    debug!("AMQPTransportConnector poll transport is none? {}", self.transport.is_none());
    let mut transport = self.transport.take().unwrap();

    //we might have received a frame before here
    transport.send_and_handle_frames()?;

    debug!("conn state: {:?}", transport.conn.state);
    if transport.conn.state == ConnectionState::Connected {
      debug!("already connected");
      return Ok(Async::Ready(transport))
    }

    trace!("waiting before poll");
    match transport.poll()? {
      Async::Ready(Some(_)) => {
        if transport.conn.state == ConnectionState::Connected {
          // Upstream had frames available and we're connected, the transport is ready
          Ok(Async::Ready(transport))
        } else {
          // Upstream had frames but we're not yet connected, continue polling
          let poll_ret = transport.poll();
          self.transport = Some(transport);
          poll_ret?;
          Ok(Async::NotReady)
        }
      },
      _ => {
        // Upstream had no frames
        self.transport = Some(transport);
        Ok(Async::NotReady)
      },
    }
  }
}

impl<T> Stream for AMQPTransport<T>
    where T: AsyncRead + AsyncWrite,
          T: Sync+Send,
          T: 'static {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<()>, io::Error> {
      trace!("stream poll");
      self.poll_heartbeat()?;
      self.poll_upstream()
    }
}

impl<T> Sink for AMQPTransport<T>
    where T: AsyncWrite,
          T: Sync+Send {
    type SinkItem = Frame;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Frame) -> StartSend<Frame, io::Error> {
        trace!("sink start send");
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        trace!("sink poll_complete");
        self.upstream.poll_complete()
    }
}
