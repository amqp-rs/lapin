use crate::{
  auth::SASLMechanism,
  types::FieldTable,
};

#[derive(Clone, Debug, PartialEq)]
#[deprecated(note = "use lapin instead")]
pub struct ConnectionProperties {
  #[deprecated(note = "use lapin instead")]
  pub mechanism:         SASLMechanism,
  #[deprecated(note = "use lapin instead")]
  pub locale:            String,
  #[deprecated(note = "use lapin instead")]
  pub client_properties: FieldTable,
}

impl Default for ConnectionProperties {
  fn default() -> Self {
    Self {
      mechanism:         SASLMechanism::default(),
      locale:            "en_US".into(),
      client_properties: FieldTable::default(),
    }
  }
}
