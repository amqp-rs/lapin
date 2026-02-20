use crate::{
    auth::AuthProvider,
    types::{AMQPValue, FieldTable, LongString, ShortString},
};
use backon::ExponentialBuilder;
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConnectionProperties {
    pub(crate) locale: ShortString,
    pub(crate) client_properties: FieldTable,
    pub(crate) auth_provider: Option<Arc<dyn AuthProvider>>,
    pub(crate) backoff: ExponentialBuilder,
    pub(crate) auto_recover: bool,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            auth_provider: None,
            backoff: ExponentialBuilder::default().with_max_times(0 /* no retry by default */),
            auto_recover: false,
        }
    }
}

impl ConnectionProperties {
    #[must_use]
    pub fn with_locale(mut self, locale: ShortString) -> Self {
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
        self.auth_provider = Some(Arc::new(provider));
        self
    }

    #[must_use]
    pub fn with_backoff(mut self, backoff: ExponentialBuilder) -> Self {
        self.backoff = backoff;
        self
    }

    #[must_use]
    pub fn configure_backoff(mut self, conf: impl Fn(&mut ExponentialBuilder)) -> Self {
        conf(&mut self.backoff);
        self
    }

    #[must_use]
    pub fn enable_auto_recover(mut self) -> Self {
        self.auto_recover = true;
        self
    }
}

impl fmt::Debug for ConnectionProperties {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionProperties")
            .field("locale", &self.locale)
            .field("client_properties", &self.client_properties)
            .field("backoff", &self.backoff)
            .field("auto_recover", &self.auto_recover)
            .finish()
    }
}
