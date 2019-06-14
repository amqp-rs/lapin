use std::fmt;

use crate::types::FieldTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionSASLMechanism {
  Plain,
  RabbitCrDemo,
}

impl Default for ConnectionSASLMechanism {
  fn default() -> Self {
    ConnectionSASLMechanism::Plain
  }
}

impl fmt::Display for ConnectionSASLMechanism {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mechanism = match self {
      ConnectionSASLMechanism::Plain        => "PLAIN",
      ConnectionSASLMechanism::RabbitCrDemo => "RABBIT-CR-DEMO",
    };
    write!(f, "{}", mechanism)
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
