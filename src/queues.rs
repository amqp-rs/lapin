use crate::{
    topology::QueueDefinition,
    types::{FieldTable, ShortString},
};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct Queues(Arc<Mutex<HashMap<ShortString, QueueDefinition>>>);

impl Queues {
    pub(crate) fn register(&self, queue: QueueDefinition) {
        let mut inner = self.0.lock();
        if let Some(q) = inner.get_mut(&queue.name) {
            q.absorb(queue);
        } else {
            inner.insert(queue.name.clone(), queue);
        }
    }

    pub(crate) fn deregister(&self, queue: &str) {
        self.0.lock().remove(queue);
    }

    pub(crate) fn topology(&self) -> Vec<QueueDefinition> {
        self.0
            .lock()
            .values()
            .filter(|q| !q.is_exclusive())
            .cloned()
            .collect()
    }

    pub(crate) fn register_binding(
        &self,
        queue: &str,
        exchange: &str,
        routing_key: &str,
        arguments: &FieldTable,
    ) {
        if let Some(queue) = self.0.lock().get_mut(queue) {
            if queue.is_exclusive() {
                queue.register_binding(exchange.into(), routing_key.into(), arguments.clone());
            }
        }
    }

    pub(crate) fn deregister_binding(
        &self,
        queue: &str,
        exchange: &str,
        routing_key: &str,
        arguments: &FieldTable,
    ) {
        if let Some(queue) = self.0.lock().get_mut(queue) {
            queue.deregister_binding(exchange.into(), routing_key.into(), arguments.clone());
        }
    }
}

impl fmt::Debug for Queues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_tuple("Queues");
        if let Some(queues) = self.0.try_lock() {
            debug.field(&*queues);
        }
        debug.finish()
    }
}
