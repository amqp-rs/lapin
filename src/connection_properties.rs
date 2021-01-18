use crate::{
    executor::{DefaultExecutor, Executor},
    reactor::ReactorBuilder,
    types::{AMQPValue, FieldTable, LongString},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn Executor>>,
    pub reactor_builder: Option<Arc<dyn ReactorBuilder>>,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            executor: None,
            reactor_builder: None,
        }
    }
}

impl ConnectionProperties {
    pub fn with_connection_name(mut self, connection_name: LongString) -> Self {
        self.client_properties.insert("connection_name".into(), AMQPValue::LongString(connection_name));
        self
    }

    pub fn with_executor<E: Executor + 'static>(mut self, executor: E) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    pub fn with_default_executor(self, max_threads: usize) -> Self {
        self.with_executor(DefaultExecutor::new(max_threads))
    }

    pub fn with_reactor<R: ReactorBuilder + 'static>(mut self, reactor_builder: R) -> Self {
        self.reactor_builder = Some(Arc::new(reactor_builder));
        self
    }
}
