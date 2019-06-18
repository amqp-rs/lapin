use futures::{Future, Poll};
use lapin_async::{
  Connect as LapinAsyncConnect, Connection,
  confirmation::Confirmation,
};

use crate::{
  Channel, ConfirmationFuture, ConnectionProperties, Error,
  uri::AMQPUri,
};

/// Connect to a server and create channels
#[derive(Clone)]
pub struct Client {
  conn: Connection,
}

impl Client {
  /// Connect to an AMQP Server
  pub fn connect(uri: &str, options: ConnectionProperties) -> ClientFuture {
    Connect::connect(uri, options)
  }

  /// Connect to an AMQP Server
  pub fn connect_uri(uri: AMQPUri, options: ConnectionProperties) -> ClientFuture {
    Connect::connect(uri, options)
  }

  /// Return a future that resolves to a `Channel` once the method succeeds
  pub fn create_channel(&self) -> impl Future<Item = Channel, Error = Error> + Send + 'static {
    Channel::create(&self.conn)
  }

  /// Register an error handler which will be called when connection reaches an Error state
  pub fn on_error<E: Fn() + Send + 'static>(&self, handler: Box<E>) {
    self.conn.on_error(handler);
  }
}

pub struct ClientFuture(ConfirmationFuture<Connection>);

impl Future for ClientFuture {
  type Item = Client;
  type Error = Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    Ok(self.0.poll()?.map(|conn| Client { conn }))
  }
}

impl From<Confirmation<Connection>> for ClientFuture {
  fn from(confirmation: Confirmation<Connection>) -> Self {
    Self(confirmation.into())
  }
}

/// Trait providing a method to connect to an AMQP server
pub trait Connect {
  /// Connect to an AMQP server
  fn connect(self, options: ConnectionProperties) -> ClientFuture;
}

impl Connect for AMQPUri {
  fn connect(self, options: ConnectionProperties) -> ClientFuture {
    LapinAsyncConnect::connect(self, options).into()
  }
}

impl Connect for &str {
  fn connect(self, options: ConnectionProperties) -> ClientFuture {
    LapinAsyncConnect::connect(self, options).into()
  }
}
