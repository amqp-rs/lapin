use crate::{reactor::FullReactor, types::{AMQPValue, FieldTable, LongString}};
use executor_trait::FullExecutor;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn FullExecutor + Send + Sync>>,
    pub reactor: Option<Arc<dyn FullReactor + Send + Sync>>,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            executor: None,
            reactor: None,
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
    pub fn with_executor<E: FullExecutor + Send + Sync + 'static>(mut self, executor: E) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    #[must_use]
    pub fn with_reactor<R: FullReactor + Send + Sync + 'static>(mut self, reactor: R) -> Self {
        self.reactor = Some(Arc::new(reactor));
        self
    }
}
