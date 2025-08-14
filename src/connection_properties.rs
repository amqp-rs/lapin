use crate::{
    recovery_config::RecoveryConfig,
    types::{AMQPValue, FieldTable, LongString},
};

#[derive(Clone)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub recovery_config: Option<RecoveryConfig>,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            recovery_config: None,
        }
    }
}

impl ConnectionProperties {
    #[must_use]
    pub fn with_connection_name(mut self, connection_name: LongString) -> Self {
        self.client_properties.insert(
            "connection_name".into(),
            AMQPValue::LongString(connection_name),
        );
        self
    }

    #[must_use]
    #[cfg(feature = "unstable")]
    pub fn with_experimental_recovery_config(mut self, config: RecoveryConfig) -> Self {
        self.recovery_config = Some(config);
        self
    }
}
