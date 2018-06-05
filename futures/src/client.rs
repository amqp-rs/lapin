use amq_protocol::uri::AMQPUri;
use lapin_async;
use lapin_async::format::frame::Frame;
use std::default::Default;
use std::io;
use std::str::FromStr;
use futures::{future,task,Async,Future,Poll,Stream};
use futures::sync::oneshot;
use tokio_io::{AsyncRead,AsyncWrite};
use tokio_timer::Interval;
use std::sync::{Arc,Mutex};
use std::time::{Duration,Instant};

use transport::*;
use channel::{Channel, ConfirmSelectOptions};

/// the Client structures connects to a server and creates channels
//#[derive(Clone)]
pub struct Client<T> {
    transport:         Arc<Mutex<AMQPTransport<T>>>,
    pub configuration: ConnectionConfiguration,
}

impl<T> Clone for Client<T>
    where T: Send {
  fn clone(&self) -> Client<T> {
    Client {
      transport:     self.transport.clone(),
      configuration: self.configuration.clone(),
    }
  }
}
#[derive(Clone,Debug,PartialEq)]
pub struct ConnectionOptions {
  pub username:  String,
  pub password:  String,
  pub vhost:     String,
  pub frame_max: u32,
  pub heartbeat: u16,
}

impl ConnectionOptions {
  pub fn from_uri(uri: AMQPUri) -> ConnectionOptions {
    ConnectionOptions {
      username: uri.authority.userinfo.username,
      password: uri.authority.userinfo.password,
      vhost: uri.vhost,
      frame_max: uri.query.frame_max.unwrap_or(0),
      heartbeat: uri.query.heartbeat.unwrap_or(0),
    }
  }
}

impl Default for ConnectionOptions {
  fn default() -> ConnectionOptions {
    ConnectionOptions {
      username:  "guest".to_string(),
      password:  "guest".to_string(),
      vhost:     "/".to_string(),
      frame_max: 0,
      heartbeat: 0,
    }
  }
}

impl FromStr for ConnectionOptions {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uri = AMQPUri::from_str(s)?;
        Ok(ConnectionOptions::from_uri(uri))
    }
}

pub type ConnectionConfiguration = lapin_async::connection::Configuration;

/// A heartbeat task.
pub struct Heartbeat {
    handle: Option<HeartbeatHandle>,
    pulse:  Box<Future<Item = (), Error = io::Error> + Send + 'static>,
}

impl Heartbeat {
    fn new<T>(transport: Arc<Mutex<AMQPTransport<T>>>, heartbeat: u16) -> Self
    where
      T: AsyncRead + AsyncWrite + Send + 'static
    {
        use self::future::{Either, Loop};

        let (tx, rx) = oneshot::channel();

        debug!("heartbeat; interval={}", heartbeat);

        let interval = if heartbeat == 0 {
            Err(())
        } else {
            Ok(Interval::new(Instant::now(), Duration::from_secs(heartbeat.into()))
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                .into_future())
        };

        let neverending = future::result(interval).or_else(|_| future::empty()).and_then(|interval| {
            future::loop_fn((interval, rx), move |(interval, rx)| {
                let transport_send = Arc::clone(&transport);
                let transport = Arc::clone(&transport);

                interval.and_then(move |(_instant, interval)| {
                    debug!("poll heartbeat");

                    future::poll_fn(move || {
                        let mut transport = lock_transport!(transport_send);
                        debug!("Sending heartbeat");
                        transport.send_frame(Frame::Heartbeat(0));
                        Ok(Async::Ready(()))
                    }).and_then(move |_| future::poll_fn(move || {
                        let mut transport = lock_transport!(transport);
                        transport.poll()
                    })).then(move |r| match r {
                        Ok(_) => Ok(interval),
                        Err(cause) => Err((cause, interval)),
                    })
                }).or_else(|(err, _interval)| {
                    error!("Error occured in heartbeat interval: {}", err);
                    Err(err)
                }).select2(rx).then(|res| {
                    match res {
                        Ok(Either::A((interval, rx)))    => Ok(Loop::Continue((interval.into_future(), rx))),
                        Ok(Either::B((_rx, _interval)))  => { trace!("stopping heartbeat"); Ok(Loop::Break(())) },
                        Err(Either::A((err, _rx)))       => Err(io::Error::new(io::ErrorKind::Other, err)),
                        Err(Either::B((err, _interval))) => Err(io::Error::new(io::ErrorKind::Other, err)),
                    }
                })
            })
        });

        Heartbeat {
            handle: Some(HeartbeatHandle(tx)),
            pulse:  Box::new(neverending),
        }
    }

    /// Get the handle for this heartbeat.
    ///
    /// As there can only be one handle for a given heartbeat task, this function can return
    /// `None` if the handle to this heartbeat was already acquired.
    pub fn handle(&mut self) -> Option<HeartbeatHandle> {
        self.handle.take()
    }
}

impl Future for Heartbeat {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.pulse.poll()
    }
}

/// A handle to stop a connection heartbeat.
pub struct HeartbeatHandle(oneshot::Sender<()>);

impl HeartbeatHandle {
    /// Signals the heartbeat task to stop sending packets to the broker.
    pub fn stop(self) {
        if let Err(_) = self.0.send(()) {
            warn!("Couldn't send stop signal to heartbeat: already gone");
        }
    }
}

impl<T: AsyncRead+AsyncWrite+Send+'static> Client<T> {
  /// Takes a stream (TCP, TLS, unix socket, etc) and uses it to connect to an AMQP server.
  ///
  /// This function returns a future that resolves once the connection handshake is done.
  /// The result is a tuple containing a `Client` that can be used to create `Channel`s and a
  /// `Heartbeat` instance. The heartbeat is a task (it implements `Future`) that should be
  /// spawned independently of the other futures.
  ///
  /// To stop the heartbeat task, see `HeartbeatHandle`.
  ///
  /// # Example
  ///
  /// ```
  /// # extern crate lapin_futures;
  /// # extern crate tokio;
  /// #
  /// # use tokio::prelude::*;
  /// #
  /// # fn main() {
  /// use tokio::net::TcpStream;
  /// use lapin_futures::client::{Client, ConnectionOptions};
  ///
  /// let addr = "127.0.0.1:5672".parse().unwrap();
  /// let f = TcpStream::connect(&addr)
  ///     .and_then(|stream| {
  ///         Client::connect(stream, ConnectionOptions::default())
  ///     })
  ///     .and_then(|(client, mut heartbeat)| {
  ///         let handle = heartbeat.handle().unwrap();
  ///         tokio::spawn(
  ///             heartbeat.map_err(|e| eprintln!("The heartbeat task errored: {}", e))
  ///         );
  ///
  ///         /// ...
  ///
  ///         handle.stop();
  ///         Ok(())
  ///     });
  /// tokio::run(
  ///     f.map_err(|e| eprintln!("An error occured: {}", e))
  /// );
  /// # }
  /// ```
  pub fn connect(stream: T, options: ConnectionOptions) -> impl Future<Item = (Self, Heartbeat), Error = io::Error> + Send + 'static {
    AMQPTransport::connect(stream, options).and_then(|transport| {
      debug!("got client service");
      let configuration = transport.conn.configuration.clone();
      let transport = Arc::new(Mutex::new(transport));
      let heartbeat = Heartbeat::new(transport.clone(), configuration.heartbeat);
      let client = Client { configuration, transport };
      Ok((client, heartbeat))
    })
  }

  /// creates a new channel
  ///
  /// returns a future that resolves to a `Channel` once the method succeeds
  pub fn create_channel(&self) -> impl Future<Item = Channel<T>, Error = io::Error> + Send + 'static {
    Channel::create(self.transport.clone())
  }

  /// returns a future that resolves to a `Channel` once the method succeeds
  /// the channel will support RabbitMQ's confirm extension
  pub fn create_confirm_channel(&self, options: ConfirmSelectOptions) -> impl Future<Item = Channel<T>, Error = io::Error> + Send + 'static {
    //FIXME: maybe the confirm channel should be a separate type
    //especially, if we implement transactions, the methods should be available on the original channel
    //but not on the confirm channel. And the basic publish method should have different results
    self.create_channel().and_then(move |channel| {
      let ch = channel.clone();

      channel.confirm_select(options).map(|_| ch)
    })
  }
}
