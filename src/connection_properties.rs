use crate::{
    auth::AuthProvider,
    recovery_config::RecoveryConfig,
    types::{AMQPValue, FieldTable, LongString, ShortString},
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ConnectionProperties {
    pub(crate) locale: String,
    pub(crate) client_properties: FieldTable,
    pub(crate) auth_provider: Option<Arc<Mutex<dyn AuthProvider>>>,
    pub(crate) recovery_config: Option<RecoveryConfig>,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            auth_provider: None,
            recovery_config: None,
        }
    }
}

impl ConnectionProperties {
    #[must_use]
    pub fn with_locale(mut self, locale: String) -> Self {
        self.locale = locale;
        self
    }

    #[must_use]
    pub fn with_client_property(mut self, key: ShortString, value: LongString) -> Self {
        self.client_properties
            .insert(key, AMQPValue::LongString(value));
        self
    }

    #[must_use]
    pub fn with_connection_name(self, connection_name: LongString) -> Self {
        self.with_client_property("connection_name".into(), connection_name)
    }

    #[must_use]
    pub fn with_auth_provider<AP: AuthProvider>(mut self, provider: AP) -> Self {
        self.auth_provider = Some(Arc::new(Mutex::new(provider)));
        self
    }

    #[must_use]
    #[cfg(feature = "unstable")]
    pub fn with_recovery_config(mut self, config: RecoveryConfig) -> Self {
        self.recovery_config = Some(config);
        self
    }
}
