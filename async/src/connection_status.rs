use parking_lot::RwLock;

use std::sync::Arc;

use crate::{
  Connection, ConnectionProperties,
  auth::Credentials,
  wait::WaitHandle,
};

#[derive(Clone, Debug, Default)]
#[deprecated(note = "use lapin instead")]
pub struct ConnectionStatus {
  inner: Arc<RwLock<Inner>>,
}

impl ConnectionStatus {
  #[deprecated(note = "use lapin instead")]
  pub fn state(&self) -> ConnectionState {
    self.inner.read().state.clone()
  }

  pub(crate) fn set_state(&self, state: ConnectionState) {
    self.inner.write().state = state
  }

  #[deprecated(note = "use lapin instead")]
  pub fn vhost(&self) -> String {
    self.inner.read().vhost.clone()
  }

  pub(crate) fn set_vhost(&self, vhost: &str) {
    self.inner.write().vhost = vhost.into();
  }

  pub(crate) fn block(&self) {
    self.inner.write().blocked = true;
  }

  pub(crate) fn unblock(&self) {
    self.inner.write().blocked = true;
  }

  #[deprecated(note = "use lapin instead")]
  pub fn blocked(&self) -> bool {
    self.inner.read().blocked
  }

  #[deprecated(note = "use lapin instead")]
  pub fn connected(&self) -> bool {
    self.inner.read().state == ConnectionState::Connected
  }

  #[deprecated(note = "use lapin instead")]
  pub fn closed(&self) -> bool {
    self.inner.read().state == ConnectionState::Closed
  }

  #[deprecated(note = "use lapin instead")]
  pub fn errored(&self) -> bool {
    self.inner.read().state == ConnectionState::Error
  }
}

#[derive(Clone, Debug)]
#[deprecated(note = "use lapin instead")]
pub enum ConnectionState {
  Initial,
  SentProtocolHeader(WaitHandle<Connection>, Credentials, ConnectionProperties),
  SentStartOk(WaitHandle<Connection>, Credentials),
  SentOpen(WaitHandle<Connection>),
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
      (ConnectionState::Initial,                ConnectionState::Initial)                => true,
      (ConnectionState::SentProtocolHeader(..), ConnectionState::SentProtocolHeader(..)) => true,
      (ConnectionState::SentStartOk(..),        ConnectionState::SentStartOk(..))        => true,
      (ConnectionState::SentOpen(_),            ConnectionState::SentOpen(_))            => true,
      (ConnectionState::Connected,              ConnectionState::Connected)              => true,
      (ConnectionState::Closing,                ConnectionState::Closing)                => true,
      (ConnectionState::Closed,                 ConnectionState::Closed)                 => true,
      (ConnectionState::Error,                  ConnectionState::Error)                  => true,
      _                                                                                  => false,
    }
  }
}

#[derive(Debug)]
struct Inner {
  state:   ConnectionState,
  vhost:   String,
  blocked: bool,
}

impl Default for Inner {
  fn default() -> Self {
    Self {
      state:   ConnectionState::default(),
      vhost:   "/".into(),
      blocked: false,
    }
  }
}
