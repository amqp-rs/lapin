/// low level wrapper for the state machine, encoding and decoding from lapin-async
use lapin_async::connection::*;
use lapin_async::format::frame::*;

use nom::{IResult,Offset};
use cookie_factory::GenError;
use bytes::BytesMut;
use std::iter::repeat;
use std::io::{self,Error,ErrorKind};
use std::time::Duration;
use futures::{Async,Poll,Sink,Stream,StartSend,Future};
use tokio_io::{AsyncRead,AsyncWrite};
use tokio_io::codec::{Decoder,Encoder,Framed};
use tokio_timer::{Interval,Timer};
use client::ConnectionOptions;

/// implements tokio-io's Decoder and Encoder
pub struct AMQPCodec;

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
      let length = buf.len();
      if length < 8192 {
        //reserve more capacity and intialize it
        buf.extend(repeat(0).take(8192 - length));
      }
      trace!("will send frame: {:?}", frame);

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
  pub upstream: Framed<T,AMQPCodec>,
  pub heartbeat: Interval,
  pub conn: Connection,
}

impl<T> AMQPTransport<T>
   where T: AsyncRead+AsyncWrite,
         T: 'static               {

  /// starts the connection process
  ///
  /// returns a future of a `AMQPTransport` that is connected
  pub fn connect(upstream: Framed<T,AMQPCodec>, options: &ConnectionOptions) -> Box<Future<Item = AMQPTransport<T>, Error = io::Error>> {
    let mut conn = Connection::new();
    conn.set_credentials(&options.username, &options.password);
    conn.set_vhost(&options.vhost);
    conn.set_heartbeat(options.heartbeat);
    conn.connect();

    let mut t = AMQPTransport {
      upstream:  upstream,
      heartbeat: Timer::default().interval(Duration::from_secs(conn.configuration.heartbeat as u64)),
      conn:      conn,
    };

    t.send_and_handle_frames();
    t.poll_children();

    let mut connector = AMQPTransportConnector {
      transport: Some(t)
    };

    trace!("pre-poll");
    connector.poll();
    trace!("post-poll");

    Box::new(connector)
  }

  pub fn poll_children(&mut self) {
    self.upstream.poll();
    self.heartbeat.poll();
  }

  pub fn send_and_handle_frames(&mut self) {
    self.send_frames();
    self.handle_frames();
  }

  pub fn send_frames(&mut self) {
    //FIXME: find a way to use a future here
    while let Some(f) = self.conn.next_frame() {
      self.upstream.start_send(f);
      self.upstream.poll_complete();
    }
    //self.upstream.poll_complete();
  }

  pub fn handle_frames(&mut self) {
    loop {
      match self.poll() {
        Ok(Async::Ready(Some(frame))) => {
          trace!("handle frames: AMQPTransport received frame: {:?}", frame);
          self.conn.handle_frame(frame);
        },
        Ok(Async::Ready(None)) => {
          trace!("handle frames: upstream poll gave Ready(None)");
          break;
        },
        Ok(Async::NotReady) => {
          trace!("handle frames: upstream poll gave NotReady");
          self.poll_children();
          break;
        },
        Err(e) => {
          error!("handle frames: upstream poll got error: {:?}", e);
          break;
        },
      };
    }
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
          T: 'static {

  type Item  = AMQPTransport<T>;
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    debug!("AMQPTransportConnector poll transport is none? {}", self.transport.is_none());
    let mut transport = self.transport.take().unwrap();

    //we might have received a frame before here
    transport.send_frames();

    debug!("conn state: {:?}", transport.conn.state);
    if transport.conn.state == ConnectionState::Connected {
      debug!("already connected");
      return Ok(Async::Ready(transport))
    }

    trace!("waiting before poll");
    transport.heartbeat.poll();
    let value = match transport.upstream.poll() {
      Ok(Async::Ready(t)) => t,
      Ok(Async::NotReady) => {
        trace!("upstream poll gave NotReady");
        transport.poll_children();
        self.transport = Some(transport);
        return Ok(Async::NotReady);
      },
      Err(e) => {
        error!("upstream poll got error: {:?}", e);
        return Err(From::from(e));
      },
    };

    match value {
      Some(frame) => {
        trace!("got frame: {:?}", frame);
        transport.conn.handle_frame(frame);
        transport.send_frames();
        transport.upstream.poll_complete();
        if transport.conn.state == ConnectionState::Connected {
          return Ok(Async::Ready(transport))
        } else {
          transport.poll_children();
          self.transport = Some(transport);
          return Ok(Async::NotReady)
        }
      },
      e => {
        error!("did not get a frame? -> {:?}", e);
        self.transport = Some(transport);
        return Ok(Async::NotReady)
      }
    }
  }
}

impl<T> Stream for AMQPTransport<T>
    where T: AsyncRead + AsyncWrite {
    type Item = Frame;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Frame>, io::Error> {
        if let Ok(Async::Ready(_)) = self.heartbeat.poll() {
            debug!("Heartbeat");
            self.start_send(Frame::Heartbeat(0));
            self.poll_complete();
        }

        trace!("stream poll");
        // and Async::NotReady.
        match try_ready!(self.upstream.poll()) {
            Some(frame) => {
              debug!("AMQPTransport received frame: {:?}", frame);
              //try!(self.poll_complete());
              return Ok(Async::Ready(Some(frame)))
            },
            None => {
              trace!("AMQPTransport returned NotReady");
              return Ok(Async::NotReady)
            }
        }
    }
}

impl<T> Sink for AMQPTransport<T>
    where T: AsyncWrite {
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

