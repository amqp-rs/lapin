use crate::{Error, ErrorKind, Result, types::ShortString, uri::AMQPUri};
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

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

    pub fn vhost(&self) -> ShortString {
        self.0.vhost.clone()
    }

    pub fn username(&self) -> String {
        self.0.username.clone()
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
        self.write().poison(err);
    }

    pub(crate) fn auto_close(&self) -> bool {
        [ConnectionState::Connecting, ConnectionState::Connected].contains(&self.state())
    }

    fn read(&self) -> RwLockReadGuard<'_, StateInner> {
        self.0.state.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, StateInner> {
        self.0.state.write().unwrap_or_else(|e| e.into_inner())
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
