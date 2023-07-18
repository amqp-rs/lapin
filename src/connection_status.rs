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
        self.0.lock().state = state;
    }

    pub(crate) fn connection_step(&self) -> Option<ConnectionStep> {
        self.0.lock().connection_step.take()
    }

    pub(crate) fn set_connection_step(&self, connection_step: ConnectionStep) {
        self.0.lock().connection_step = Some(connection_step);
    }

    pub(crate) fn connection_resolver(&self) -> Option<PromiseResolver<Connection>> {
        let resolver = self.0.lock().connection_resolver();
        // We carry the Connection here to drop the lock() above before dropping the Connection
        resolver.map(|(resolver, _connection)| resolver)
    }

    pub(crate) fn connection_step_name(&self) -> Option<&'static str> {
        self.0.lock().connection_step_name()
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

    pub(crate) fn auto_close(&self) -> bool {
        [ConnectionState::Connecting, ConnectionState::Connected].contains(&self.0.lock().state)
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

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ConnectionState {
    #[default]
    Initial,
    Connecting,
    Connected,
    Closing,
    Closed,
    Error,
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
    fn connection_resolver(&mut self) -> Option<(PromiseResolver<Connection>, Option<Connection>)> {
        if let ConnectionState::Connecting = self.state {
            self.connection_step
                .take()
                .map(|connection_step| match connection_step {
                    ConnectionStep::ProtocolHeader(resolver, connection, ..) => {
                        (resolver, Some(connection))
                    }
                    ConnectionStep::StartOk(resolver, connection, ..) => {
                        (resolver, Some(connection))
                    }
                    ConnectionStep::Open(resolver, ..) => (resolver, None),
                })
        } else {
            None
        }
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
