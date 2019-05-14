use amq_protocol::sasl;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credentials {
  username: String,
  password: String,
}

impl Credentials {
  pub fn new(username: String, password: String) -> Self {
    Self { username, password }
  }

  pub fn sasl_plain_auth_string(&self) -> String {
    sasl::plain_auth_string(&self.username, &self.password)
  }
}

impl Default for Credentials {
  fn default() -> Self {
    Self::new("guest".into(), "guest".into())
  }
}

