use crate::{executor::Executor, reactor::Reactor, types::FieldTable};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn Executor>>,
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
    pub fn with_executor<E: Executor + 'static>(mut self, executor: E) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    pub fn with_reactor<R: Reactor + Send + Sync + 'static>(mut self, reactor: R) -> Self {
        self.reactor = Some(Arc::new(reactor));
        self
    }
}
