use lapin_async::connection::*;
use lapin_async::api::ChannelState;
use lapin_async::format::*;
use lapin_async::format::frame::*;

use nom::{IResult,Offset};
use cookie_factory::GenError;
use std::io::{self,Error,ErrorKind,Read,Write};
use futures::{Async,AsyncSink,Poll,Sink,Stream,StartSend,Future};
use futures::future::{self, FutureResult,Loop};
use std::rc::Rc;
use std::cell::RefCell;
use tokio_core::io::{Codec,EasyBuf,Framed,Io};
use tokio_service::Service;
use tokio_proto::pipeline::{ClientProto, ClientService};
use tokio_proto::TcpClient;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpStream;
use std::net::SocketAddr;

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

pub struct AMQPProto {
  //pub conn: Connection<'static>
}

impl AMQPProto {
  pub fn new() -> AMQPProto {
    AMQPProto {
      //conn: Connection::new(),
    }
  }
}

impl<T: Io + 'static> ClientProto<T> for AMQPProto {
    type Request  = Frame;
    type Response = Frame;

    /// `Framed<T, LineCodec>` is the return value of `io.framed(LineCodec)`
    type Transport = AMQPTransport<Framed<T, AMQPCodec>>;
    //type BindTransport = Box<Future<Item = Self::Transport, Error = io::Error>>;//Result<Self::Transport, io::Error>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
      let mut t = AMQPTransport {
        upstream: io.framed(AMQPCodec),
        conn: Connection::new(),
      };
      t.connect();
      Ok(t)
    }
}

pub struct AMQPTransport<T> {
  pub upstream: T,
  pub conn: Connection<'static>,
}

impl<T> AMQPTransport<T>
    where T: Stream<Item = Frame, Error = io::Error>,
          T: Sink<SinkItem = Frame, SinkError = io::Error> {
  pub fn connect(&mut self) {
    self.conn.connect();
    let frame = self.conn.frame_queue.pop_front().unwrap();
    self.start_send(frame);//.unwrap().wait();
    self.upstream.poll_complete();
  }
}

impl<T> Stream for AMQPTransport<T>
    where T: Stream<Item = Frame, Error = io::Error>,
          T: Sink<SinkItem = Frame, SinkError = io::Error>,
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
              return Ok(Async::NotReady)
            }
        }
            /*
        loop {
            // Poll the upstream transport. `try_ready!` will bubble up errors
            // and Async::NotReady.
            match try_ready!(self.upstream.poll()) {
                Some(ref frame) => {
                  println!("AMQPTransport received frame: {:?}", frame);
                  try!(self.poll_complete());
                },
                None => {
                  return Ok(Async::NotReady)
                }
            }
        }
        */
    }
}

impl<T> Sink for AMQPTransport<T>
    where T: Sink<SinkItem = Frame, SinkError = io::Error>,
{
    type SinkItem = Frame;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Frame) -> StartSend<Frame, io::Error> {
        println!("sink start send");
        // Only accept the write if there are no pending pongs
        /*if self.pongs_remaining > 0 {
            return Ok(AsyncSink::NotReady(item));
        }
        */

        // If there are no pending pongs, then send the item upstream
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        println!("sink poll_complete");
        /*
        while self.pongs_remaining > 0 {
            // Try to send the pong upstream
            let res = try!(self.upstream.start_send("[pong]".to_string()));

            if !res.is_ready() {
                // The upstream is not ready to accept new items
                break;
            }

            // The pong has been sent upstream
            self.pongs_remaining -= 1;
        }
        */

        // Call poll_complete on the upstream
        //
        // If there are remaining pongs to send, this call may create additional
        // capacity. One option could be to attempt to send the pongs again.
        // However, if a `start_send` returned NotReady, and this poll_complete
        // *did* create additional capacity in the upstream, then *our*
        // `poll_complete` will get called again shortly.
        //
        // Hopefully this makes sense... it probably doesn't, so please ask
        // questions in the Gitter channel and help me explain this better :)
        self.upstream.poll_complete()
    }
}


/*
    fn bind_transport(&self, io: T) -> Self::BindTransport {
      let mut t = AMQPTransport {
        upstream: io.framed(AMQPCodec),
        conn: Connection::new(),
      };
      t.connect();
      Ok(t)
    }
*/
pub struct Client {
    upstream: Framed<TcpStream, AMQPCodec>, //AMQPTransport<TcpStream>,
    conn:  Connection<'static>,
}

impl Client {
  pub fn connect(addr: &SocketAddr, handle: &Handle) -> Box<Future<Item = Client, Error = io::Error>> {
    let ret = TcpStream::connect(addr, handle)
      .map(|stream| {
        stream.framed(AMQPCodec)
      }).and_then(|transport| {
        println!("got client service");
        let mut client = Client {
          upstream: transport,
          conn:     Connection::new(),
        };

        future::ok(client)
      });

    Box::new(ret)

  }
  /*
  pub fn connect(addr: &SocketAddr, handle: &Handle) -> Box<Future<Item = Client, Error = io::Error>> {
    let ret = TcpStream::connect(addr, handle)
      .map(|stream| {
        stream.framed(AMQPCodec)
      }).and_then(|transport| {
        println!("got client service");
        let mut client = Client {
          upstream: transport,
          conn:     Connection::new(),
        };

        client.conn.connect();
        let f = future::loop_fn(
          client, |mut cl| {
            let Client { upstream: upstream, conn: conn } = cl;

            println!("loop[{}]", line!());
            let next_frame = conn.frame_queue.pop_front();

            println!("loop[{}] will poll", line!());
            //future::ok(f).then(|_| {
            //upstream.poll().map(|frame| {
            upstream.map(|frame| {
              println!("loop[{}] polling got frame: {:?}", line!(), frame);
              //if let Async::Ready(Some(fr)) = frame {
                conn.handle_frame(frame);
              //}
              upstream
            }).map(|upstream| {
              next_frame.map(|frame| upstream.send(frame)).unwrap_or(upstream)
              /*if let Some(frame) = next_frame {
                println!("loop[{}] got frame {:?}", line!(), frame);
                upstream.send(frame)
              }*/
            }).map(|upstream| {

            let client = Client { upstream: upstream, conn: conn };
            let res : FutureResult<Loop<Client,Client>,Error> = future::ok(if client.conn.state == ConnectionState::Connected {
              Loop::Break(client)
            } else {
              Loop::Continue(client)
            });
            res
            })
        });
        f
          //client.handshake().map(|_| future::ok(client))
      });

      println!("ending Client::connect");
      Box::new(ret)
      //ret
    }
        */

    pub fn run(&mut self) {
      
    }

    pub fn handshake(&mut self) -> Box<Future<Item = (), Error = io::Error>> {
      Box::new(future::ok(()))
    }

    /*
    pub fn ping(&self) -> Box<Future<Item = (), Error = io::Error>> {
        // The `call` response future includes the string, but since this is a
        // "ping" request, we don't really need to include the "pong" response
        // string.
        println!("IN PING");
        let resp = self.call(Frame::Heartbeat(0))
            .and_then(|resp| {
                println!("got response: {:?}", resp);
                Ok(())
            });

        // Box the response future because we are lazy and don't want to define
        // a new future type and `impl T` isn't stable yet...
        Box::new(resp)
    }
    */
}

/*
pub struct Client {
    inner: ClientService<TcpStream, AMQPProto>,
    conn:  Connection<'static>,
}

impl Client {
    pub fn connect(addr: &SocketAddr, handle: &Handle) -> Box<Future<Item = Client, Error = io::Error>> {
        let ret = TcpClient::new(AMQPProto::new())
            .connect(addr, handle)
            .map(|client_service| {
                println!("got client service");
                let client = Client {
                  inner: client_service,
                  conn:  Connection::new(),
                };

                client.conn.connect();
                futures::loop_fn(
                  client, |client| {

                  })
                client.handshake().map(|_| future::ok(client))
            });

        println!("ending Client::connect");
        Box::new(ret)
    }

    pub fn handshake(&mut self) -> Box<Future<Item = (), Error = io::Error>> {
      future::ok(())
    }

    pub fn ping(&self) -> Box<Future<Item = (), Error = io::Error>> {
        // The `call` response future includes the string, but since this is a
        // "ping" request, we don't really need to include the "pong" response
        // string.
        println!("IN PING");
        let resp = self.call(Frame::Heartbeat(0))
            .and_then(|resp| {
                println!("got response: {:?}", resp);
                Ok(())
            });

        // Box the response future because we are lazy and don't want to define
        // a new future type and `impl T` isn't stable yet...
        Box::new(resp)
    }
}
*/

/*
impl Service for Client {
    type Request  = Frame;
    type Response = Frame;
    type Error = io::Error;
    // For simplicity, box the future.
    type Future = Box<Future<Item = Frame, Error = io::Error>>;

    fn call(&self, req: Frame) -> Self::Future {
        Box::new(self.inner.call(req))
    }
}
*/
/*
pub struct InnerClient<'consumer, T:Read+Write+Io> {
  pub connection: Connection<'consumer>,
  pub stream:     T,
}

impl<'consumer, T:Read+Write+Io> InnerClient<'consumer, T> {
  pub fn run(&mut self) -> Result<ConnectionState> {
    self.stream.poll_read();
    self.connection.run(&mut self.stream)
  }

  pub fn connect(&mut self) {
    self.connection.connect(&mut self.stream);
  }

  pub fn poll_io(&mut self) -> Async<()> {
    self.stream.poll_read()
  }
}

pub struct Client<'consumer, T:Read+Write+Io> {
  pub inner: Rc<RefCell<InnerClient<'consumer, T>>>
}

impl<'consumer, T:Read+Write+Io+'consumer> Client<'consumer, T> {
  pub fn new(stream: T) -> Box<Future<Item=Client<'consumer, T>, Error=Error>+'consumer> {
    let mut client = Client {
        inner: Rc::new(RefCell::new(InnerClient {
        connection: Connection::new(),
        stream:     stream,
      }))
    };

    let connector = Connector { client: Some(client) };
    println!("created connector");
    Box::new(connector)
  }

  pub fn create_channel(&mut self) -> Box<Future<Item=Channel<'consumer, T>, Error=Error>+'consumer> {
    Box::new(ChannelOpener::new(
      self.inner.clone()
    ))
  }
}

pub struct Connector<'consumer, T:Read+Write+Io> {
  client: Option<Client<'consumer, T>>,
}

impl<'consumer, T:Read+Write+Io> Future for Connector<'consumer, T> {
  type Item  = Client<'consumer, T>;
  type Error = Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut client = self.client.take().unwrap();

    let state = client.inner.borrow().connection.state;

    println!("POLL state = {:?}", state);
    match state {
      ConnectionState::Initial => {
        let res = {
          let mut inner = client.inner.borrow_mut();
          inner.connect();
          inner.run()
        };
        match res {
          Ok(ConnectionState::Connected) => {
            println!("returning a client");
            return Ok(Async::Ready(client));
          },
          Ok(_) => {
            self.client = Some(client);
            Ok(Async::NotReady)
          },
          Err(e) => return Err(e),
        }
      }
      ConnectionState::Connecting(_) => {
        let res = {
          let mut inner = client.inner.borrow_mut();
          inner.run()
        };
        match res {
          Ok(ConnectionState::Connected) => {
            println!("returning a client");
            return Ok(Async::Ready(client));
          },
          Ok(_) => {
            self.client = Some(client);
            Ok(Async::NotReady)
          },
          Err(e) => {return Err(e);},
        }
      }
      s => {
        Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
      }
    }
  }
}

pub struct Channel<'consumer, T:Read+Write+Io> {
  pub inner: Rc<RefCell<InnerClient<'consumer, T>>>,
  pub id:    u16,
}

pub struct ChannelOpener<'consumer, T:Read+Write+Io> {
  channel: Option<Channel<'consumer, T>>,
}

impl<'consumer, T:Read+Write+Io> ChannelOpener<'consumer, T> {
  pub fn new(client: Rc<RefCell<InnerClient<'consumer, T>>>) -> ChannelOpener<'consumer, T> {
    println!("creating channel opener");
    let id = {
      let mut inner = client.borrow_mut();
      let id = inner.connection.create_channel();
      //FIXME
      inner.connection.channel_open(id, "".to_string()).expect("channel_open");
      id
    };

    ChannelOpener {
      channel: Some(Channel {
                 inner: client,
                 id:    id,
               })
    }
  }
}

impl<'consumer, T:Read+Write+Io> Future for ChannelOpener<'consumer, T> {
  type Item  = Channel<'consumer, T>;
  type Error = Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut channel = self.channel.take().unwrap();

    let state: ChannelState = channel.inner.borrow().connection.get_state(channel.id).unwrap();

    println!("channel opener poll: state = {:?}", state);

    match state {
      ChannelState::Initial | ChannelState::AwaitingChannelOpenOk => {
        let res = {
          let mut inner = channel.inner.borrow_mut();
          inner.poll_io();
          inner.run()
        };
        match res {
          Ok(ConnectionState::Connected) => {
            let channel_state = channel.inner.borrow().connection.get_state(channel.id).expect("there should be a state");
            match channel_state {
              ChannelState::Initial | ChannelState::AwaitingChannelOpenOk => {
                Ok(Async::NotReady)
              },
              ChannelState::Connected => {
                return Ok(Async::Ready(channel));
              },
              s => {
                Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
              }
            }
          },
          s => {
            Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
          }
        }
      },
      ChannelState::Connected => {
        return Ok(Async::Ready(channel));
      },
      s => {
        Err(Error::new(ErrorKind::ConnectionAborted, format!("could not connect: state={:?}", s)))
      }
    }
  }
}
*/

