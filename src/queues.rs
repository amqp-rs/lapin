use crate::{
    queue::QueueState,
    topology::QueueDefinition,
    types::{FieldTable, ShortString},
};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct Queues(Arc<Mutex<HashMap<ShortString, QueueState>>>);

impl Queues {
    pub(crate) fn register(&self, queue: QueueState) {
        // QueueState tracks the consumers associated with a queue.
        //
        // If a queue is re-declared (e.g. to get the number of outstanding messages)
        // we do not want to _replace_ the entry in the `queues` hashmap.
        // If we do replace it we lose track of what consumers are associated with that queue:
        // we have no way to error/cancel them, we have no way to send them incoming messages.
        //
        // This can be avoided with an "insert-if-missing" operation.
        let mut inner = self.0.lock();
        let name = queue.name();
        if let Some(q) = inner.get_mut(&name) {
            q.absorb(queue);
        } else {
            inner.insert(name, queue);
        }
    }

    pub(crate) fn deregister(&self, queue: &str) {
        self.0.lock().remove(queue);
    }

    pub(crate) fn topology(&self) -> Vec<QueueDefinition> {
        self.0
            .lock()
            .values()
            .filter_map(QueueState::topology)
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
