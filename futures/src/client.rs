use amq_protocol::uri::AMQPUri;
use futures::{future, Future, Poll, Stream};
use lapin_async;
use log::{debug, error, warn};
use parking_lot::Mutex;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_sync::oneshot;
use tokio_timer::Interval;

use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::channel::{Channel, ConfirmSelectOptions};
use crate::error::{Error, ErrorKind};
use crate::transport::*;

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
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uri = AMQPUri::from_str(s).map_err(|e| ErrorKind::InvalidUri(e))?;
        Ok(ConnectionOptions::from_uri(uri))
    }
}

pub type ConnectionConfiguration = lapin_async::connection::Configuration;

fn heartbeat_pulse<T: AsyncRead+AsyncWrite+Send+'static>(transport: Arc<Mutex<AMQPTransport<T>>>, heartbeat: u16, rx: oneshot::Receiver<()>) -> impl Future<Item = (), Error = Error> + Send + 'static {
    let interval  = if heartbeat == 0 {
        Err(())
    } else {
        Ok(Interval::new(Instant::now(), Duration::from_secs(heartbeat.into()))
           .map_err(|e| ErrorKind::HeartbeatTimer(e).into()))
    };

    future::select_all(vec![
        future::Either::A(rx.map(|_| debug!("Stopping heartbeat")).or_else(|_| future::empty())),
        future::Either::B(future::result(interval).or_else(|_| future::empty()).and_then(move |interval| {
            interval.for_each(move |_| {
                debug!("poll heartbeat");

                let transport = transport.clone();

                future::poll_fn(move || {
                    let mut transport = transport.lock();
                    debug!("Sending heartbeat");
                    transport.send_heartbeat()
                }).map(|_| ()).map_err(|err| {
                    error!("Error occured in heartbeat interval: {}", err);
                    err
                })
            })
        })),
    ]).map(|_| ()).map_err(|(err, ..)| err)
}

/// A heartbeat task.
pub struct Heartbeat<Pulse> {
    handle: Option<HeartbeatHandle>,
    pulse:  Pulse,
}

impl<Pulse> Heartbeat<Pulse> {
    /// Get the handle for this heartbeat.
    ///
    /// As there can only be one handle for a given heartbeat task, this function can return
    /// `None` if the handle to this heartbeat was already acquired.
    pub fn handle(&mut self) -> Option<HeartbeatHandle> {
        self.handle.take()
    }
}

fn make_heartbeat<F, Pulse>(pulse_maker: F) -> Heartbeat<Pulse> where F: FnOnce(oneshot::Receiver<()>) -> Pulse {
    let (tx, rx) = oneshot::channel();

    Heartbeat {
        handle: Some(HeartbeatHandle(tx)),
        pulse:  pulse_maker(rx),
    }
}

impl<F> Future for Heartbeat<F> where F: Future {
    type Item = F::Item;
    type Error = F::Error;

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

impl<T: AsyncRead+AsyncWrite+Send+Sync+'static> Client<T> {
  /// Takes a stream (TCP, TLS, unix socket, etc) and uses it to connect to an AMQP server.
  ///
  /// This function returns a future that resolves once the connection handshake is done.
  /// The result is a tuple containing a `Client` that can be used to create `Channel`s and a
  /// `Heartbeat` instance. The heartbeat is a task (it implements `Future`) that should be
  /// spawned independently of the other futures.
  ///
  /// To stop the heartbeat task, see `HeartbeatHandle`.
  pub fn connect(stream: T, options: ConnectionOptions) ->
    impl Future<Item = (Self, Heartbeat<impl Future<Item = (), Error = Error> + Send + 'static>), Error = Error> + Send + 'static
  {
    AMQPTransport::connect(stream, options).and_then(|transport| {
      debug!("got client service");
      let configuration = transport.conn.configuration.clone();
      let transport = Arc::new(Mutex::new(transport));
      // The configured value is the timeout, not the interval.
      // rabbitmq-server uses half that time as the periodicity for the heartbeat.
      // Let's do the same.
      let heartbeat_interval =  configuration.heartbeat / 2;
      let heartbeat = make_heartbeat(|rx| {
        debug!("heartbeat; timeout={}; interval={}", configuration.heartbeat, heartbeat_interval);

        heartbeat_pulse(transport.clone(), heartbeat_interval, rx)
      });
      let client = Client { configuration, transport };
      Ok((client, heartbeat))
    })
  }

  /// creates a new channel
  ///
  /// returns a future that resolves to a `Channel` once the method succeeds
  pub fn create_channel(&self) -> impl Future<Item = Channel<T>, Error = Error> + Send + 'static {
    Channel::create(self.transport.clone())
  }

  /// returns a future that resolves to a `Channel` once the method succeeds
  /// the channel will support RabbitMQ's confirm extension
  pub fn create_confirm_channel(&self, options: ConfirmSelectOptions) -> impl Future<Item = Channel<T>, Error = Error> + Send + 'static {
    //FIXME: maybe the confirm channel should be a separate type
    //especially, if we implement transactions, the methods should be available on the original channel
    //but not on the confirm channel. And the basic publish method should have different results
    self.create_channel().and_then(move |channel| {
      let ch = channel.clone();

      channel.confirm_select(options).map(|_| ch)
    })
  }
}
