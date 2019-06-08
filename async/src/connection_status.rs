use parking_lot::RwLock;

use std::sync::Arc;

use crate::{
  connection::Connection,
  connection_properties::ConnectionProperties,
  credentials::Credentials,
  wait::WaitHandle,
};

#[derive(Clone, Debug, Default)]
pub struct ConnectionStatus {
  inner: Arc<RwLock<Inner>>,
}

impl ConnectionStatus {
  pub fn state(&self) -> ConnectionState {
    self.inner.read().state.clone()
  }

  pub fn set_state(&self, state: ConnectionState) {
    self.inner.write().state = state
  }

  pub fn set_vhost(&self, vhost: &str) {
    self.inner.write().vhost = vhost.into();
  }

  pub fn vhost(&self) -> String {
    self.inner.read().vhost.clone()
  }

  pub fn block(&self) {
    self.inner.write().blocked = true;
  }

  pub fn unblock(&self) {
    self.inner.write().blocked = true;
  }

  pub fn blocked(&self) -> bool {
    self.inner.read().blocked
  }

  pub fn connected(&self) -> bool {
    self.inner.read().state == ConnectionState::Connected
  }
}

#[derive(Clone, Debug)]
pub enum ConnectionState {
  Initial,
  SentProtocolHeader(WaitHandle<Connection>, Credentials, ConnectionProperties),
  SentStartOk(WaitHandle<Connection>),
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
      (ConnectionState::SentStartOk(_),         ConnectionState::SentStartOk(_))         => true,
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
