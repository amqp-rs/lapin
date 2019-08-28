use futures::{Future, Poll};
use lapin::{confirmation::Confirmation, Connect as LapinConnect, Connection};

use crate::{
    tcp::Identity, uri::AMQPUri, Channel, ConfirmationFuture, ConnectionProperties, Error,
};

/// Connect to a server and create channels
#[derive(Clone)]
pub struct Client {
    conn: Connection,
}

impl Client {
    /// Connect to an AMQP Server
    pub fn connect(uri: &str, options: ConnectionProperties) -> ClientFuture {
        Connect::connect(uri, options, None)
    }

    /// Connect to an AMQP Server
    pub fn connect_with_identity(
        uri: &str,
        options: ConnectionProperties,
        identity: Identity<'_, '_>,
    ) -> ClientFuture {
        Connect::connect(uri, options, Some(identity))
    }

    /// Connect to an AMQP Server
    pub fn connect_uri(uri: AMQPUri, options: ConnectionProperties) -> ClientFuture {
        Connect::connect(uri, options, None)
    }

    /// Connect to an AMQP Server
    pub fn connect_uri_with_identity(
        uri: AMQPUri,
        options: ConnectionProperties,
        identity: Identity<'_, '_>,
    ) -> ClientFuture {
        Connect::connect(uri, options, Some(identity))
    }

    /// Return a future that resolves to a `Channel` once the method succeeds
    pub fn create_channel(&self) -> impl Future<Item = Channel, Error = Error> + Send + 'static {
        Channel::create(&self.conn)
    }

    /// Update the secret used by some authentication module such as oauth2
    pub fn update_secret(&self, new_secret: &str, reason: &str) -> ConfirmationFuture<()> {
        self.conn.update_secret(new_secret, reason).into()
    }

    /// Block all consumers and publishers on this connection
    pub fn block(&self, reason: &str) -> ConfirmationFuture<()> {
        self.conn.block(reason).into()
    }

    /// Unblock all consumers and publishers on this connection
    pub fn unblock(&self) -> ConfirmationFuture<()> {
        self.conn.unblock().into()
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
    fn connect(
        self,
        options: ConnectionProperties,
        identity: Option<Identity<'_, '_>>,
    ) -> ClientFuture;
}

impl Connect for AMQPUri {
    fn connect(
        self,
        options: ConnectionProperties,
        identity: Option<Identity<'_, '_>>,
    ) -> ClientFuture {
        LapinConnect::connect(self, options, identity).into()
    }
}

impl Connect for &str {
    fn connect(
        self,
        options: ConnectionProperties,
        identity: Option<Identity<'_, '_>>,
    ) -> ClientFuture {
        LapinConnect::connect(self, options, identity).into()
    }
}
