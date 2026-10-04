use crate::{Error, ErrorKind, Result, types::ShortString, uri::AMQPUri};
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// Shared, cheaply cloneable view of a connection's current state.
///
/// Obtained from [`Connection::status`].
///
/// [`Connection::status`]: crate::Connection::status
#[derive(Clone, Default)]
pub struct ConnectionStatus(Arc<Inner>);

impl ConnectionStatus {
    pub(crate) fn new(uri: &AMQPUri) -> Self {
        Self(Arc::new(Inner {
            vhost: uri.vhost.as_str().into(),
            username: uri.authority.userinfo.username.clone(),
            state: RwLock::new(StateInner::default()),
        }))
    }

    pub(crate) fn state(&self) -> ConnectionState {
        self.read().state
    }

    pub(crate) fn set_state(&self, state: ConnectionState) -> ConnectionState {
        let mut inner = self.write();
        std::mem::replace(&mut inner.state, state)
    }

    pub(crate) fn set_connecting(&self) -> Result<()> {
        self.write().set_connecting()
    }

    pub(crate) fn set_reconnecting(&self) {
        self.write().set_reconnecting();
    }

    /// The virtual host this connection is attached to.
    #[must_use]
    pub fn vhost(&self) -> ShortString {
        self.0.vhost.clone()
    }

    /// The username used to authenticate this connection.
    #[must_use]
    pub fn username(&self) -> String {
        self.0.username.clone()
    }

    pub(crate) fn block(&self) {
        self.write().blocked = true;
    }

    pub(crate) fn unblock(&self) {
        self.write().blocked = false;
    }

    /// Returns `true` if the broker has issued a `Connection.Blocked` notice.
    #[must_use]
    pub fn blocked(&self) -> bool {
        self.read().blocked
    }

    /// Returns `true` if the connection is in [`ConnectionState::Connected`].
    #[must_use]
    pub fn connected(&self) -> bool {
        self.state() == ConnectionState::Connected
    }

    pub(crate) fn ensure_connected(&self) -> Result<()> {
        if !self.connected() {
            return Err(ErrorKind::InvalidConnectionState(self.state()).into());
        }
        Ok(())
    }

    /// Returns `true` if the connection is in [`ConnectionState::Connecting`].
    #[must_use]
    pub fn connecting(&self) -> bool {
        self.state() == ConnectionState::Connecting
    }

    /// Returns `true` if the connection is in [`ConnectionState::Reconnecting`].
    #[must_use]
    pub fn reconnecting(&self) -> bool {
        self.state() == ConnectionState::Reconnecting
    }

    /// Returns `true` if the connection is in [`ConnectionState::Closing`].
    #[must_use]
    pub fn closing(&self) -> bool {
        self.state() == ConnectionState::Closing
    }

    /// Returns `true` if the connection is in [`ConnectionState::Closed`].
    #[must_use]
    pub fn closed(&self) -> bool {
        self.state() == ConnectionState::Closed
    }

    /// Returns `true` if the connection is in [`ConnectionState::Error`].
    #[must_use]
    pub fn errored(&self) -> bool {
        self.state() == ConnectionState::Error
    }

    pub(crate) fn poison(&self, err: Error) {
        self.write().poison(err);
    }

    pub(crate) fn auto_close(&self) -> bool {
        [
            ConnectionState::Connecting,
            ConnectionState::Connected,
            ConnectionState::Reconnecting,
        ]
        .contains(&self.state())
    }

    fn read(&self) -> RwLockReadGuard<'_, StateInner> {
        self.0.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, StateInner> {
        self.0.state.write().unwrap_or_else(|e| e.into_inner())
    }
}

/// The lifecycle state of an AMQP connection.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ConnectionState {
    /// The connection object has been created but the TCP handshake has not started.
    #[default]
    Initial,
    /// The TCP connection is open and the AMQP handshake is in progress.
    Connecting,
    /// The AMQP handshake completed successfully; the connection is ready.
    Connected,
    /// `Connection.Close` has been sent; waiting for `Connection.Close-Ok`.
    Closing,
    /// The connection has been closed normally.
    Closed,
    /// The connection was lost and is being re-established (auto-recovery).
    Reconnecting,
    /// The connection has been closed due to a protocol or IO error.
    Error,
}

impl fmt::Debug for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ConnectionStatus");
        debug
            .field("vhost", &self.0.vhost)
            .field("username", &self.0.username);
        if let Ok(inner) = self.0.state.try_read() {
            debug
                .field("state", &inner.state)
                .field("blocked", &inner.blocked);
        }
        debug.finish()
    }
}

struct Inner {
    vhost: ShortString,
    username: String,
    state: RwLock<StateInner>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            vhost: "/".into(),
            username: "guest".into(),
            state: RwLock::new(StateInner::default()),
        }
    }
}

#[derive(Default)]
struct StateInner {
    state: ConnectionState,
    blocked: bool,
    poison: Option<Error>,
}

impl StateInner {
    fn set_connecting(&mut self) -> Result<()> {
        if matches!(
            self.state,
            ConnectionState::Closing | ConnectionState::Closed | ConnectionState::Error
        ) {
            return Err(ErrorKind::InvalidConnectionState(self.state).into());
        }
        self.state = ConnectionState::Connecting;
        self.poison.take().map(Err).unwrap_or(Ok(()))
    }

    fn set_reconnecting(&mut self) {
        let _ = self.poison.take();
        self.state = ConnectionState::Reconnecting;
        self.blocked = false;
    }

    fn poison(&mut self, err: Error) {
        self.poison = Some(err);
    }
}
