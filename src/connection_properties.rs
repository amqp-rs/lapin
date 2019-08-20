use crate::{auth::SASLMechanism, types::FieldTable};

#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionProperties {
    pub mechanism: SASLMechanism,
    pub locale: String,
    pub client_properties: FieldTable,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            mechanism: SASLMechanism::default(),
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
        }
    }
}
