use std::fmt;

use crate::types::FieldTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionSASLMechanism {
  PLAIN,
}

impl Default for ConnectionSASLMechanism {
  fn default() -> Self {
    ConnectionSASLMechanism::PLAIN
  }
}

impl fmt::Display for ConnectionSASLMechanism {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionProperties {
  pub mechanism:         ConnectionSASLMechanism,
  pub locale:            String,
  pub client_properties: FieldTable,
}

impl Default for ConnectionProperties {
  fn default() -> Self {
    Self {
      mechanism:         ConnectionSASLMechanism::default(),
      locale:            "en_US".into(),
      client_properties: FieldTable::default(),
    }
  }
}
