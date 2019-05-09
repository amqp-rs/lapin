use parking_lot::RwLock;

use std::sync::Arc;

use crate::{
  connection_properties::ConnectionProperties,
  credentials::Credentials,
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

  pub fn set_error(&self) {
    self.set_state(ConnectionState::Error);
  }

  pub fn set_connecting_state(&self, state: ConnectingState) {
    self.set_state(ConnectionState::Connecting(state))
  }

  pub fn set_vhost(&self, vhost: &str) {
    self.inner.write().vhost = vhost.to_string();
  }

  pub fn vhost(&self) -> String {
    self.inner.read().vhost.clone()
  }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
  Initial,
  Connecting(ConnectingState),
  Connected,
  Closing,
  Closed,
  Error,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectingState {
  Initial,
  SentProtocolHeader(Credentials, ConnectionProperties),
  SentStartOk,
  SentOpen,
}

impl Default for ConnectionState {
  fn default() -> Self {
    ConnectionState::Initial
  }
}

#[derive(Debug)]
struct Inner {
  state: ConnectionState,
  vhost: String,
}

impl Default for Inner {
  fn default() -> Self {
    Self {
      state: ConnectionState::default(),
      vhost: "/".to_string(),
    }
  }
}
