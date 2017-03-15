use lapin_async::connection::*;
use lapin_async::api::{ChannelState,RequestId};
use lapin_async::format::*;
use lapin_async::format::frame::*;

use nom::{IResult,Offset};
use cookie_factory::GenError;
use std::io::{self,Error,ErrorKind,Read,Write};
use futures::{Async,AsyncSink,Poll,Sink,Stream,StartSend,Future};
use futures::future::{self, FutureResult,Loop};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use tokio_core::io::{Codec,EasyBuf,Framed,Io};
use tokio_service::Service;
use tokio_proto::pipeline::{ClientProto, ClientService};
use tokio_proto::TcpClient;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpStream;
use std::net::SocketAddr;
use std::thread;
use std::sync::{Arc,Mutex};

pub struct AMQPCodec;

impl Codec for AMQPCodec {
    type In = Frame;
    type Out = Frame;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Frame>, io::Error> {
        let (consumed, f) = match frame(buf.as_slice()) {
          IResult::Incomplete(_) => {
            return Ok(None)
          },
          IResult::Error(e) => {
            return Err(io::Error::new(io::ErrorKind::Other, format!("parse error: {:?}", e)))
          },
          IResult::Done(i, frame) => {
            (buf.as_slice().offset(i), frame)
          }
        };

        println!("decoded frame: {:?}", f);
        buf.drain_to(consumed);
        Ok(Some(f))
    }

    fn encode(&mut self, frame: Frame, buf: &mut Vec<u8>) -> io::Result<()> {
      if buf.len() < 8192 {
        buf.resize(8192, 0);
      }
      println!("will send frame: {:?}", frame);
      loop {
        let gen_res = match &frame {
          &Frame::ProtocolHeader => {
            gen_protocol_header((buf.as_mut_slice(), 0)).map(|tup| tup.1)
          },
          &Frame::Heartbeat(_) => {
            gen_heartbeat_frame((buf.as_mut_slice(), 0)).map(|tup| tup.1)
          },
          &Frame::Method(channel, ref method) => {
            gen_method_frame((buf.as_mut_slice(), 0), channel, method).map(|tup| tup.1)
          },
          &Frame::Header(channel_id, class_id, ref header) => {
            gen_content_header_frame((buf.as_mut_slice(), 0), channel_id, class_id, header.body_size).map(|tup| tup.1)
          },
          &Frame::Body(channel_id, ref data) => {
            gen_content_body_frame((buf.as_mut_slice(), 0), channel_id, data).map(|tup| tup.1)
          }
        };

        match gen_res {
          Ok(sz) => {
            buf.truncate(sz);
            println!("serialized frame: {} bytes", sz);
            return Ok(());
          },
          Err(e) => {
            println!("error generating frame: {:?}", e);
            match e {
              GenError::BufferTooSmall(sz) => {
                buf.resize(sz, 0);
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

pub struct AMQPTransport<T> {
  pub upstream: Framed<T,AMQPCodec>,
  pub conn: Connection,
}

impl<T> AMQPTransport<T>
    where //T: Stream<Item = Frame, Error = io::Error>,
          //T: Sink<SinkItem = Frame, SinkError = io::Error>,
          T : Io,
          T: 'static {
  pub fn connect(upstream: Framed<T,AMQPCodec>) -> Box<Future<Item = AMQPTransport<T>, Error = io::Error>> {
    let mut t = AMQPTransport {
      upstream: upstream,
      conn:     Connection::new(),
    };

    t.conn.connect();
    let f = t.conn.next_frame().unwrap();
    t.upstream.start_send(f);
    t.upstream.poll_complete();
    t.upstream.get_mut().poll_read();

    let mut connector = AMQPTransportConnector {
      transport: Some(t)
    };

    println!("pre-poll");
    connector.poll();
    println!("post-poll");

    Box::new(connector)
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
    match self.poll() {
      Ok(Async::Ready(Some(frame))) => {
        println!("handle frames: AMQPTransport received frame: {:?}", frame);
        self.conn.handle_frame(frame);
      },
      Ok(Async::Ready(None)) => {
        println!("handle frames: upstream poll gave Ready(None)");
      },
      Ok(Async::NotReady) => {
        println!("handle frames: upstream poll gave NotReady");
        self.upstream.get_mut().poll_read();
      },
      Err(e) => {
        println!("handle frames: upstream poll got error: {:?}", e);
      },
    };
  }
}

pub struct AMQPTransportConnector<T> {
  pub transport: Option<AMQPTransport<T>>,
}

impl<T> Future for AMQPTransportConnector<T>
    where //T: Stream<Item = Frame, Error = io::Error>,
          //T: Sink<SinkItem = Frame, SinkError = io::Error> {
          T : Io {

  type Item  = AMQPTransport<T>;
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    println!("AMQPTransportConnector poll transport is none? {}", self.transport.is_none());
    let mut transport = self.transport.take().unwrap();
    println!("conn state: {:?}", transport.conn.state);
    if transport.conn.state == ConnectionState::Connected {
      println!("already connected");
      return Ok(Async::Ready(transport))
    }

    println!("waiting before poll");
    //thread::sleep_ms(2000);
   
    let value = match transport.upstream.poll() {
      Ok(Async::Ready(t)) => t,
      Ok(Async::NotReady) => {
        println!("upstream poll gave NotReady");
        thread::sleep_ms(100);
        //continue;
        transport.upstream.get_mut().poll_read();
        self.transport = Some(transport);
        return Ok(Async::NotReady);
      },
      Err(e) => {
        println!("upstream poll got error: {:?}", e);
        return Err(From::from(e));
      },
    };

    match value {
    //match try_ready!(transport.upstream.poll()) {
      Some(frame) => {
        println!("got frame: {:?}", frame);
        transport.conn.handle_frame(frame);
        while let Some(f) = transport.conn.next_frame() {
          transport.upstream.start_send(f);
          transport.upstream.poll_complete();
        }
        transport.upstream.poll_complete();
        if transport.conn.state == ConnectionState::Connected {
          return Ok(Async::Ready(transport))
        } else {
          self.transport = Some(transport);
          return Ok(Async::NotReady)
        }
      },
      e => {
        println!("did not get a frame? -> {:?}", e);
        self.transport = Some(transport);
        return Ok(Async::NotReady)
      }
    }
  }
}

impl<T> Stream for AMQPTransport<T>
    where //T: Stream<Item = Frame, Error = io::Error>,
          //T: Sink<SinkItem = Frame, SinkError = io::Error>,
          T : Io,
{
    type Item = Frame;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Frame>, io::Error> {
        println!("stream poll");
        // and Async::NotReady.
        match try_ready!(self.upstream.poll()) {
            Some(frame) => {
              println!("AMQPTransport received frame: {:?}", frame);
              //try!(self.poll_complete());
              return Ok(Async::Ready(Some(frame)))
            },
            None => {
              println!("AMQPTransport returned NotReady");
              return Ok(Async::NotReady)
            }
        }
    }
}

impl<T> Sink for AMQPTransport<T>
    where //T: Sink<SinkItem = Frame, SinkError = io::Error>,
          T : Io,
{
    type SinkItem = Frame;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Frame) -> StartSend<Frame, io::Error> {
        println!("sink start send");
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        println!("sink poll_complete");
        self.upstream.poll_complete()
    }
}


#[derive(Clone)]
pub struct Client {
    transport: Arc<Mutex<AMQPTransport<TcpStream>>>,
}

impl Client {
  pub fn connect(addr: &SocketAddr, handle: &Handle) -> Box<Future<Item = Client, Error = io::Error>> {
    let ret = TcpStream::connect(addr, handle)
      .and_then(|stream| {
        println!("will connect AMQPTransport");
        AMQPTransport::connect(stream.framed(AMQPCodec))
      }).and_then(|transport| {
        println!("got client service");


        //transport.connect();
        let mut client = Client {
          transport: Arc::new(Mutex::new(transport)),
        };

        future::ok(client)
      });

    Box::new(ret)
  }

  pub fn create_channel(&self) -> Box<Future<Item = Channel, Error = io::Error>> {
    let channel_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      let channel_id: u16 = transport.conn.create_channel();
      match transport.conn.channel_open(channel_id, "".to_string()) {
        //FIXME: should use errors from underlying library here
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not create channel")))
        ),
        Ok(request_id) => {
          println!("request id: {}", request_id);
          transport.send_frames();
          transport.handle_frames();

          //FIXME: very afterwards that the state is Connected and not error
          Box::new(wait_for_answer(channel_transport.clone(), request_id).map(move |_| {
            Channel {
              id:        channel_id,
              transport: channel_transport,
            }
          }))
        }
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

}

#[derive(Clone)]
pub struct Channel {
  pub transport: Arc<Mutex<AMQPTransport<TcpStream>>>,
  pub id:    u16,
}

impl Channel {
  pub fn queue_declare(&self, name: &str) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_declare(self.id, 0, name.to_string(), false, false, false, false, false, HashMap::new()) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not declare queue")))
        ),
        Ok(request_id) => {
          println!("queue_declare request id: {}", request_id);
          transport.send_frames();

          transport.handle_frames();

          println!("queue_declare returning closure");
          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  pub fn basic_publish(&self, queue: &str, payload: &[u8]) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.basic_publish(self.id, 0, "".to_string(), queue.to_string(), false, false) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish")))
        ),
        Ok(_) => {
          transport.send_frames();
          transport.conn.send_content_frames(self.id, 60, payload);
          transport.send_frames();

          transport.handle_frames();

          Box::new(future::ok(()))
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }
}

pub fn wait_for_answer(transport: Arc<Mutex<AMQPTransport<TcpStream>>>, request_id: RequestId) -> Box<Future<Item = (), Error = io::Error>> {
  Box::new(future::poll_fn(move || {
    //println!("polling queue_declare closure");
    let connected = if let Ok(mut tr) = transport.try_lock() {
      if ! tr.conn.is_finished(request_id) {
        //retry because we might have obtained a new frame
        tr.handle_frames();
        tr.conn.is_finished(request_id)
      } else {
        true
      }
    } else {
      return Ok(Async::NotReady);
    };

    if connected {
      //println!("queue_declare closure returning ready");
      Ok(Async::Ready(()))
    } else {
      //println!("queue_declare closure returning not ready");
      Ok(Async::NotReady)
    }
  }))

}
