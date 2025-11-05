use crate::{
    Connection, ConnectionProperties, Error, PromiseResolver, Result,
    auth::{Credentials, SASLMechanism},
    uri::AMQPUri,
};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Default)]
pub struct ConnectionStatus(Arc<Mutex<Inner>>);

impl ConnectionStatus {
    pub(crate) fn new(uri: &AMQPUri) -> Self {
        let status = Self::default();
        status.set_vhost(&uri.vhost);
        status.set_username(&uri.authority.userinfo.username);
        status
    }

    pub fn state(&self) -> ConnectionState {
        self.lock_inner().state
    }

    pub(crate) fn set_state(&self, state: ConnectionState) -> ConnectionState {
        let mut inner = self.lock_inner();
        std::mem::replace(&mut inner.state, state)
    }

    pub(crate) fn set_connecting(
        &self,
        resolver_out: PromiseResolver<()>,
        resolver_in: PromiseResolver<Connection>,
        conn: Connection,
        creds: Credentials,
        mechanism: SASLMechanism,
        options: ConnectionProperties,
    ) -> Result<()> {
        self.lock_inner()
            .set_connecting(resolver_out, resolver_in, conn, creds, mechanism, options)
    }

    pub(crate) fn set_reconnecting(&self) {
        self.lock_inner().set_reconnecting();
    }

    pub(crate) fn connection_step(&self) -> Option<ConnectionStep> {
        self.lock_inner().connection_step.take()
    }

    pub(crate) fn set_connection_step(&self, connection_step: ConnectionStep) {
        self.lock_inner().connection_step = Some(connection_step);
    }

    pub(crate) fn connection_resolver(&self) -> Option<PromiseResolver<Connection>> {
        let resolver = self.lock_inner().connection_resolver();
        // We carry the Connection here to drop the lock() above before dropping the Connection
        resolver.map(|(resolver, _connection)| resolver.resolver_in)
    }

    pub fn vhost(&self) -> String {
        self.lock_inner().vhost.clone()
    }

    pub(crate) fn set_vhost(&self, vhost: &str) {
        self.lock_inner().vhost = vhost.into();
    }

    pub fn username(&self) -> String {
        self.lock_inner().username.clone()
    }

    pub(crate) fn set_username(&self, username: &str) {
        self.lock_inner().username = username.into();
    }

    pub(crate) fn block(&self) {
        self.lock_inner().blocked = true;
    }

    pub(crate) fn unblock(&self) {
        self.lock_inner().blocked = false;
    }

    pub fn blocked(&self) -> bool {
        self.lock_inner().blocked
    }

    pub fn connected(&self) -> bool {
        self.lock_inner().state == ConnectionState::Connected
    }

    pub fn connecting(&self) -> bool {
        self.lock_inner().state == ConnectionState::Connecting
    }

    pub fn reconnecting(&self) -> bool {
        self.lock_inner().state == ConnectionState::Reconnecting
    }

    pub fn closing(&self) -> bool {
        self.lock_inner().state == ConnectionState::Closing
    }

    pub fn closed(&self) -> bool {
        self.lock_inner().state == ConnectionState::Closed
    }

    pub fn errored(&self) -> bool {
        self.lock_inner().state == ConnectionState::Error
    }

    pub(crate) fn poison(&self, err: Error) {
        if let Some((resolver, _connection)) = self.lock_inner().poison(err.clone()) {
            // We perform the reject here to drop the lock() above before dropping the Connection
            resolver.reject(err);
        }
    }

    pub(crate) fn auto_close(&self) -> bool {
        [ConnectionState::Connecting, ConnectionState::Connected].contains(&self.lock_inner().state)
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub(crate) struct ConnectionResolver {
    resolver_out: Option<PromiseResolver<()>>,
    resolver_in: PromiseResolver<Connection>,
}

impl ConnectionResolver {
    pub(crate) fn resolve(&self, conn: Connection) {
        self.resolver_in.resolve(conn);
    }

    pub(crate) fn reject(&self, err: Error) {
        if let Some(resolver) = self.resolver_out.as_ref() {
            resolver.reject(err.clone());
        }
        self.resolver_in.reject(err);
    }
}

pub(crate) enum ConnectionStep {
    ProtocolHeader(
        ConnectionResolver,
        Connection,
        Credentials,
        SASLMechanism,
        ConnectionProperties,
    ),
    StartOk(ConnectionResolver, Connection, Credentials),
    SecureOk(ConnectionResolver, Connection),
    Open(ConnectionResolver),
}

impl ConnectionStep {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            ConnectionStep::ProtocolHeader(..) => "ProtocolHeader",
            ConnectionStep::StartOk(..) => "StartOk",
            ConnectionStep::SecureOk(..) => "SecureOk",
            ConnectionStep::Open(..) => "Open",
        }
    }

    pub(crate) fn into_connection_resolver(self) -> (ConnectionResolver, Option<Connection>) {
        match self {
            ConnectionStep::ProtocolHeader(resolver, connection, ..) => {
                (resolver, Some(connection))
            }
            ConnectionStep::StartOk(resolver, connection, ..) => (resolver, Some(connection)),
            ConnectionStep::SecureOk(resolver, connection) => (resolver, Some(connection)),
            ConnectionStep::Open(resolver, ..) => (resolver, None),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ConnectionState {
    #[default]
    Initial,
    Connecting,
    Connected,
    Closing,
    Closed,
    Reconnecting,
    Error,
}

impl fmt::Debug for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ConnectionStatus");
        if let Ok(inner) = self.0.try_lock() {
            debug
                .field("state", &inner.state)
                .field("vhost", &inner.vhost)
                .field("username", &inner.username)
                .field("blocked", &inner.blocked);
        }
        debug.finish()
    }
}

struct Inner {
    connection_step: Option<ConnectionStep>,
    state: ConnectionState,
    vhost: String,
    username: String,
    blocked: bool,
    poison: Option<Error>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            connection_step: None,
            state: ConnectionState::default(),
            vhost: "/".into(),
            username: "guest".into(),
            blocked: false,
            poison: None,
        }
    }
}

impl Inner {
    fn set_connecting(
        &mut self,
        resolver_out: PromiseResolver<()>,
        resolver_in: PromiseResolver<Connection>,
        conn: Connection,
        creds: Credentials,
        mechanism: SASLMechanism,
        options: ConnectionProperties,
    ) -> Result<()> {
        self.state = ConnectionState::Connecting;
        self.connection_step = Some(ConnectionStep::ProtocolHeader(
            ConnectionResolver {
                resolver_out: Some(resolver_out),
                resolver_in,
            },
            conn,
            creds,
            mechanism,
            options,
        ));
        self.poison.take().map(Err).unwrap_or(Ok(()))
    }

    fn set_reconnecting(&mut self) {
        let _ = self.connection_step.take();
        let _ = self.poison.take();
        self.state = ConnectionState::Reconnecting;
        self.blocked = false;
    }

    fn connection_resolver(&mut self) -> Option<(ConnectionResolver, Option<Connection>)> {
        self.connection_step
            .take()
            .map(ConnectionStep::into_connection_resolver)
    }

    fn poison(&mut self, err: Error) -> Option<(ConnectionResolver, Option<Connection>)> {
        self.poison = Some(err);

        self.connection_resolver()
    }
}
