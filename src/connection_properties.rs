use crate::{executor::Executor, types::FieldTable};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn Executor>>,
    pub max_executor_threads: usize,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            executor: None,
            max_executor_threads: 1,
        }
    }
}
