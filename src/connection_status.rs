use crate::{
    Connection, ConnectionProperties, PromiseResolver,
    auth::{Credentials, SASLMechanism},
};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Default)]
pub struct ConnectionStatus(Arc<Mutex<Inner>>);

impl ConnectionStatus {
    pub fn state(&self) -> ConnectionState {
        self.lock_inner().state
    }

    pub(crate) fn set_state(&self, state: ConnectionState) -> ConnectionState {
        let mut inner = self.lock_inner();
        std::mem::replace(&mut inner.state, state)
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
        resolver.map(|(resolver, _connection)| resolver)
    }

    pub(crate) fn connection_step_name(&self) -> Option<&'static str> {
        self.lock_inner().connection_step_name()
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

    pub fn closing(&self) -> bool {
        self.lock_inner().state == ConnectionState::Closing
    }

    pub fn closed(&self) -> bool {
        self.lock_inner().state == ConnectionState::Closed
    }

    pub fn errored(&self) -> bool {
        self.lock_inner().state == ConnectionState::Error
    }

    pub(crate) fn auto_close(&self) -> bool {
        [ConnectionState::Connecting, ConnectionState::Connected].contains(&self.lock_inner().state)
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub(crate) enum ConnectionStep {
    ProtocolHeader(
        PromiseResolver<Connection>,
        Connection,
        Credentials,
        SASLMechanism,
        ConnectionProperties,
    ),
    StartOk(PromiseResolver<Connection>, Connection, Credentials),
    Open(PromiseResolver<Connection>),
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
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            connection_step: None,
            state: ConnectionState::default(),
            vhost: "/".into(),
            username: "guest".into(),
            blocked: false,
        }
    }
}

impl Inner {
    fn set_reconnecting(&mut self) {
        if let Some(step) = self.connection_step.take() {
            // FIXME: how do we handle this? Reconnection during (re)connection
            drop(step);
        }
        self.state = ConnectionState::Reconnecting;
        self.blocked = false;
    }

    fn connection_resolver(&mut self) -> Option<(PromiseResolver<Connection>, Option<Connection>)> {
        self.connection_step
            .take()
            .map(|connection_step| match connection_step {
                ConnectionStep::ProtocolHeader(resolver, connection, ..) => {
                    (resolver, Some(connection))
                }
                ConnectionStep::StartOk(resolver, connection, ..) => (resolver, Some(connection)),
                ConnectionStep::Open(resolver, ..) => (resolver, None),
            })
    }

    fn connection_step_name(&self) -> Option<&'static str> {
        if let ConnectionState::Connecting = self.state {
            self.connection_step.as_ref().map(|step| match step {
                ConnectionStep::ProtocolHeader(..) => "ProtocolHeader",
                ConnectionStep::StartOk(..) => "StartOk",
                ConnectionStep::Open(..) => "Open",
            })
        } else {
            None
        }
    }
}
