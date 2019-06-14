use amq_protocol::sasl;

use crate::connection_properties::ConnectionSASLMechanism;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credentials {
  username: String,
  password: String,
}

impl Credentials {
  pub fn new(username: String, password: String) -> Self {
    Self { username, password }
  }

  pub fn username(&self) -> &str {
    &self.username
  }

  pub fn password(&self) -> &str {
    &self.password
  }

  pub fn auth_string(&self, mechanism: ConnectionSASLMechanism) -> String {
    match mechanism {
      ConnectionSASLMechanism::Plain        => sasl::plain_auth_string(&self.username, &self.password),
      ConnectionSASLMechanism::RabbitCrDemo => self.username.clone(),
    }
  }

  pub fn rabbit_cr_demo_answer(&self) -> String {
    format!("My password is {}", self.password)
  }
}

impl Default for Credentials {
  fn default() -> Self {
    Self::new("guest".into(), "guest".into())
  }
}
