use crate::{
    executor::{DefaultExecutor, Executor},
    types::FieldTable,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn Executor>>,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            executor: None,
        }
    }
}

impl ConnectionProperties {
    pub fn with_executor<E: Executor + 'static>(mut self, executor: E) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    pub fn with_default_executor(self, max_threads: usize) -> Self {
        self.with_executor(DefaultExecutor::new(max_threads))
    }
}
