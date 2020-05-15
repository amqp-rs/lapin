use crate::{
    auth::{Credentials, SASLMechanism},
    Connection, ConnectionProperties, PromiseResolver,
};
use parking_lot::Mutex;
use std::{fmt, sync::Arc};

#[derive(Clone, Default)]
pub struct ConnectionStatus(Arc<Mutex<Inner>>);

impl ConnectionStatus {
    pub fn state(&self) -> ConnectionState {
        self.0.lock().state.clone()
    }

    pub(crate) fn set_state(&self, state: ConnectionState) {
        self.0.lock().state = state
    }

    pub(crate) fn connection_step(&self) -> Option<ConnectionStep> {
        self.0.lock().connection_step.take()
    }

    pub(crate) fn set_connection_step(&self, connection_step: ConnectionStep) {
        self.0.lock().connection_step = Some(connection_step);
    }

    pub(crate) fn connection_resolver(&self) -> Option<PromiseResolver<Connection>> {
        self.0.lock().connection_resolver()
    }

    pub fn vhost(&self) -> String {
        self.0.lock().vhost.clone()
    }

    pub(crate) fn set_vhost(&self, vhost: &str) {
        self.0.lock().vhost = vhost.into();
    }

    pub fn username(&self) -> String {
        self.0.lock().username.clone()
    }

    pub(crate) fn set_username(&self, username: &str) {
        self.0.lock().username = username.into();
    }

    pub(crate) fn block(&self) {
        self.0.lock().blocked = true;
    }

    pub(crate) fn unblock(&self) {
        self.0.lock().blocked = false;
    }

    pub fn blocked(&self) -> bool {
        self.0.lock().blocked
    }

    pub fn connected(&self) -> bool {
        self.0.lock().state == ConnectionState::Connected
    }

    pub fn closing(&self) -> bool {
        self.0.lock().state == ConnectionState::Closing
    }

    pub fn closed(&self) -> bool {
        self.0.lock().state == ConnectionState::Closed
    }

    pub fn errored(&self) -> bool {
        self.0.lock().state == ConnectionState::Error
    }
}

#[allow(clippy::large_enum_variant)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Initial,
    Connecting,
    Connected,
    Closing,
    Closed,
    Error,
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::Initial
    }
}

impl fmt::Debug for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ConnectionStatus");
        if let Some(inner) = self.0.try_lock() {
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
    fn connection_resolver(&mut self) -> Option<PromiseResolver<Connection>> {
        if let ConnectionState::Connecting = self.state {
            self.connection_step
                .take()
                .map(|connection_step| match connection_step {
                    ConnectionStep::ProtocolHeader(resolver, ..) => resolver,
                    ConnectionStep::StartOk(resolver, ..) => resolver,
                    ConnectionStep::Open(resolver, ..) => resolver,
                })
        } else {
            None
        }
    }
}
