use crate::types::FieldTable;
use executor_trait::Executor;
use reactor_trait::Reactor;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn Executor + Send + Sync>>,
    pub reactor: Option<Arc<dyn Reactor + Send + Sync>>,
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
    pub fn with_executor<E: Executor + Send + Sync + 'static>(mut self, executor: E) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    pub fn with_reactor<R: Reactor + Send + Sync + 'static>(mut self, reactor: R) -> Self {
        self.reactor = Some(Arc::new(reactor));
        self
    }
}
