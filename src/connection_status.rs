use crate::{
    Connection, Error, ErrorKind, PromiseResolver, Result, auth::AuthProvider, types::ShortString,
    uri::AMQPUri,
};
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

#[derive(Clone, Default)]
pub struct ConnectionStatus(Arc<RwLock<Inner>>);

impl ConnectionStatus {
    pub(crate) fn new(uri: &AMQPUri) -> Self {
        let status = Self::default();
        status.set_vhost(&uri.vhost);
        status.set_username(&uri.authority.userinfo.username);
        status
    }

    pub(crate) fn state(&self) -> ConnectionState {
        self.read().state
    }

    pub(crate) fn set_state(&self, state: ConnectionState) -> ConnectionState {
        let mut inner = self.write();
        std::mem::replace(&mut inner.state, state)
    }

    pub(crate) fn set_connecting(
        &self,
        resolver: PromiseResolver<Connection>,
        conn: Connection,
    ) -> Result<()> {
        self.write().set_connecting(resolver, conn)
    }

    pub(crate) fn set_reconnecting(&self) {
        self.write().set_reconnecting();
    }

    pub(crate) fn connection_step(&self) -> Option<ConnectionStep> {
        self.write().connection_step.take()
    }

    pub(crate) fn set_connection_step(&self, connection_step: ConnectionStep) {
        self.write().connection_step = Some(connection_step);
    }

    pub(crate) fn connection_resolver(&self) -> Option<PromiseResolver<Connection>> {
        let resolver = self.write().connection_resolver();
        // We carry the Connection here to drop the lock() above before dropping the Connection
        resolver.map(|(resolver, _connection)| resolver)
    }

    pub fn vhost(&self) -> ShortString {
        self.read().vhost.clone()
    }

    pub(crate) fn set_vhost(&self, vhost: &str) {
        self.write().vhost = vhost.into();
    }

    pub fn username(&self) -> String {
        self.read().username.clone()
    }

    pub(crate) fn set_username(&self, username: &str) {
        self.write().username = username.into();
    }

    pub(crate) fn block(&self) {
        self.write().blocked = true;
    }

    pub(crate) fn unblock(&self) {
        self.write().blocked = false;
    }

    pub fn blocked(&self) -> bool {
        self.read().blocked
    }

    pub fn connected(&self) -> bool {
        self.state() == ConnectionState::Connected
    }

    pub(crate) fn ensure_connected(&self) -> Result<()> {
        if !self.connected() {
            return Err(ErrorKind::InvalidConnectionState(self.state()).into());
        }
        Ok(())
    }

    pub fn connecting(&self) -> bool {
        self.state() == ConnectionState::Connecting
    }

    pub fn reconnecting(&self) -> bool {
        self.state() == ConnectionState::Reconnecting
    }

    pub fn closing(&self) -> bool {
        self.state() == ConnectionState::Closing
    }

    pub fn closed(&self) -> bool {
        self.state() == ConnectionState::Closed
    }

    pub fn errored(&self) -> bool {
        self.state() == ConnectionState::Error
    }

    pub(crate) fn poison(&self, err: Error) {
        let resolver = self.write().poison(err.clone());
        // We carry the Connection here to drop the lock() above before dropping the Connection
        if let Some((resolver, _connection)) = resolver {
            resolver.reject(err);
        }
    }

    pub(crate) fn auto_close(&self) -> bool {
        [ConnectionState::Connecting, ConnectionState::Connected].contains(&self.state())
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.0.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.0.write().unwrap_or_else(|e| e.into_inner())
    }
}

pub(crate) enum ConnectionStep {
    ProtocolHeader(PromiseResolver<Connection>, Connection),
    StartOk(
        PromiseResolver<Connection>,
        Connection,
        Arc<dyn AuthProvider>,
    ),
    SecureOk(
        PromiseResolver<Connection>,
        Connection,
        Arc<dyn AuthProvider>,
    ),
    Open(PromiseResolver<Connection>),
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

    pub(crate) fn into_connection_resolver(
        self,
    ) -> (PromiseResolver<Connection>, Option<Connection>) {
        match self {
            ConnectionStep::ProtocolHeader(resolver, connection, ..) => {
                (resolver, Some(connection))
            }
            ConnectionStep::StartOk(resolver, connection, ..) => (resolver, Some(connection)),
            ConnectionStep::SecureOk(resolver, connection, ..) => (resolver, Some(connection)),
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
        if let Ok(inner) = self.0.try_read() {
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
    vhost: ShortString,
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
        resolver: PromiseResolver<Connection>,
        conn: Connection,
    ) -> Result<()> {
        self.state = ConnectionState::Connecting;
        self.connection_step = Some(ConnectionStep::ProtocolHeader(resolver, conn));
        self.poison.take().map(Err).unwrap_or(Ok(()))
    }

    fn set_reconnecting(&mut self) {
        let _ = self.connection_step.take();
        let _ = self.poison.take();
        self.state = ConnectionState::Reconnecting;
        self.blocked = false;
    }

    fn connection_resolver(&mut self) -> Option<(PromiseResolver<Connection>, Option<Connection>)> {
        self.connection_step
            .take()
            .map(ConnectionStep::into_connection_resolver)
    }

    fn poison(&mut self, err: Error) -> Option<(PromiseResolver<Connection>, Option<Connection>)> {
        self.poison = Some(err);
        self.connection_resolver()
    }
}
