use lapin_async;
use std::default::Default;
use std::io;
use futures::{future,Future};
use tokio_io::{AsyncRead,AsyncWrite};
use std::sync::{Arc,Mutex};

use transport::*;
use channel::{Channel, ConfirmSelectOptions};

/// the Client structures connects to a server and creates channels
#[derive(Clone)]
pub struct Client<T> {
    transport:         Arc<Mutex<AMQPTransport<T>>>,
    pub configuration: ConnectionConfiguration,
}

#[derive(Clone,Debug,PartialEq)]
pub struct ConnectionOptions {
  pub username:  String,
  pub password:  String,
  pub vhost:     String,
  pub frame_max: u32,
  pub heartbeat: u16,
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

pub type ConnectionConfiguration = lapin_async::connection::Configuration;

impl<T: AsyncRead+AsyncWrite+Sync+Send+'static> Client<T> {
  /// takes a stream (TCP, TLS, unix socket, etc) and uses it to connect to an AMQP server.
  ///
  /// this method returns a future that resolves once the connection handshake is done.
  /// The result is a client that can be used to create a channel
  pub fn connect(stream: T, options: &ConnectionOptions) -> Box<Future<Item = Client<T>, Error = io::Error>> {
    Box::new(AMQPTransport::connect(stream, options).and_then(|mut transport| {
      debug!("got client service");
      transport.start_heartbeat();
      let config = transport.conn.configuration.clone();
      let client = Client {
        transport:     Arc::new(Mutex::new(transport)),
        configuration: config,
      };

      future::ok(client)
    }))
  }

  /// creates a new channel
  ///
  /// returns a future that resolves to a `Channel` once the method succeeds
  pub fn create_channel(&self) -> Box<Future<Item = Channel<T>, Error = io::Error>> {
    Channel::create(self.transport.clone())
  }

  /// returns a future that resolves to a `Channel` once the method succeeds
  /// the channel will support RabbitMQ's confirm extension
  pub fn create_confirm_channel(&self, options: ConfirmSelectOptions) -> Box<Future<Item = Channel<T>, Error = io::Error>> {

    //FIXME: maybe the confirm channel should be a separate type
    //especially, if we implement transactions, the methods should be available on the original channel
    //but not on the confirm channel. And the basic publish method should have different results
    Box::new(self.create_channel().and_then(move |channel| {
      let ch = channel.clone();

      channel.confirm_select(&options).map(|_| ch)
    }))
  }
}
