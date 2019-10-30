use crate::{auth::Credentials, pinky_swear::Pinky, Connection, ConnectionProperties, Result};
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct ConnectionStatus {
    inner: Arc<RwLock<Inner>>,
}

impl ConnectionStatus {
    pub fn state(&self) -> ConnectionState {
        self.inner.read().state.clone()
    }

    pub(crate) fn set_state(&self, state: ConnectionState) {
        self.inner.write().state = state
    }

    pub fn vhost(&self) -> String {
        self.inner.read().vhost.clone()
    }

    pub(crate) fn set_vhost(&self, vhost: &str) {
        self.inner.write().vhost = vhost.into();
    }

    pub fn username(&self) -> String {
        self.inner.read().username.clone()
    }

    pub(crate) fn set_username(&self, username: &str) {
        self.inner.write().username = username.into();
    }

    pub(crate) fn block(&self) {
        self.inner.write().blocked = true;
    }

    pub(crate) fn unblock(&self) {
        self.inner.write().blocked = false;
    }

    pub fn blocked(&self) -> bool {
        self.inner.read().blocked
    }

    pub fn connected(&self) -> bool {
        self.inner.read().state == ConnectionState::Connected
    }

    pub fn closing(&self) -> bool {
        self.inner.read().state == ConnectionState::Closing
    }

    pub fn closed(&self) -> bool {
        self.inner.read().state == ConnectionState::Closed
    }

    pub fn errored(&self) -> bool {
        self.inner.read().state == ConnectionState::Error
    }
}

#[derive(Clone, Debug)]
pub enum ConnectionState {
    Initial,
    SentProtocolHeader(
        Pinky<Result<Connection>, Result<()>>,
        Credentials,
        ConnectionProperties,
    ),
    SentStartOk(Pinky<Result<Connection>, Result<()>>, Credentials),
    SentOpen(Pinky<Result<Connection>, Result<()>>),
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

impl PartialEq for ConnectionState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ConnectionState::Initial, ConnectionState::Initial) => true,
            (ConnectionState::SentProtocolHeader(..), ConnectionState::SentProtocolHeader(..)) => {
                true
            }
            (ConnectionState::SentStartOk(..), ConnectionState::SentStartOk(..)) => true,
            (ConnectionState::SentOpen(_), ConnectionState::SentOpen(_)) => true,
            (ConnectionState::Connected, ConnectionState::Connected) => true,
            (ConnectionState::Closing, ConnectionState::Closing) => true,
            (ConnectionState::Closed, ConnectionState::Closed) => true,
            (ConnectionState::Error, ConnectionState::Error) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
struct Inner {
    state: ConnectionState,
    vhost: String,
    username: String,
    blocked: bool,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            state: ConnectionState::default(),
            vhost: "/".into(),
            username: "guest".into(),
            blocked: false,
        }
    }
}
