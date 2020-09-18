use crate::{
    consumer::Consumer,
    message::{BasicGetMessage, Delivery},
    queue::{Queue, QueueState},
    types::ShortString,
    BasicProperties, Channel, Error, PromiseResolver,
};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct Queues {
    queues: Arc<Mutex<HashMap<ShortString, QueueState>>>,
}

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
        self.queues.lock().entry(queue.name()).or_insert(queue);
    }

    pub(crate) fn deregister(&self, queue: &str) {
        self.queues.lock().remove(queue);
    }

    fn with_queue<F: FnOnce(&mut QueueState)>(&self, queue: &str, f: F) {
        f(self
            .queues
            .lock()
            .entry(queue.into())
            .or_insert_with(|| Queue::new(queue.into(), 0, 0).into()))
    }

    pub(crate) fn register_consumer(
        &self,
        queue: &str,
        consumer_tag: ShortString,
        consumer: Consumer,
    ) {
        self.with_queue(queue, |queue| {
            queue.register_consumer(consumer_tag, consumer);
        });
    }

    pub(crate) fn deregister_consumer(&self, consumer_tag: &str) {
        for queue in self.queues.lock().values_mut() {
            queue.deregister_consumer(consumer_tag);
        }
    }

    pub(crate) fn drop_prefetched_messages(&self) {
        for queue in self.queues.lock().values() {
            queue.drop_prefetched_messages();
        }
    }

    pub(crate) fn cancel_consumers(&self) {
        for queue in self.queues.lock().values() {
            queue.cancel_consumers();
        }
    }

    pub(crate) fn error_consumers(&self, error: Error) {
        for queue in self.queues.lock().values() {
            queue.error_consumers(error.clone());
        }
    }

    pub(crate) fn start_consumer_delivery(
        &self,
        consumer_tag: &str,
        message: Delivery,
    ) -> Option<ShortString> {
        for queue in self.queues.lock().values_mut() {
            if let Some(consumer) = queue.get_consumer(consumer_tag) {
                consumer.start_new_delivery(message);
                return Some(queue.name());
            }
        }
        None
    }

    pub(crate) fn start_basic_get_delivery(
        &self,
        queue: &str,
        message: BasicGetMessage,
        resolver: PromiseResolver<Option<BasicGetMessage>>,
    ) {
        self.with_queue(queue, |queue| {
            queue.start_new_delivery(message, resolver);
        });
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        channel: &Channel,
        queue: &str,
        consumer_tag: Option<ShortString>,
        size: u64,
        properties: BasicProperties,
    ) {
        self.with_queue(queue, |queue| match consumer_tag {
            Some(consumer_tag) => {
                if let Some(consumer) = queue.get_consumer(&consumer_tag) {
                    consumer.set_delivery_properties(properties);
                    if size == 0 {
                        consumer.new_delivery_complete(channel.clone());
                    }
                }
            }
            None => {
                queue.set_delivery_properties(properties);
                if size == 0 {
                    queue.new_delivery_complete();
                }
            }
        })
    }

    pub(crate) fn handle_body_frame(
        &self,
        channel: &Channel,
        queue: &str,
        consumer_tag: Option<ShortString>,
        remaining_size: usize,
        payload: Vec<u8>,
    ) {
        self.with_queue(queue, |queue| match consumer_tag {
            Some(consumer_tag) => {
                if let Some(consumer) = queue.get_consumer(&consumer_tag) {
                    consumer.receive_delivery_content(payload);
                    if remaining_size == 0 {
                        consumer.new_delivery_complete(channel.clone());
                    }
                }
            }
            None => {
                queue.receive_delivery_content(payload);
                if remaining_size == 0 {
                    queue.new_delivery_complete();
                }
            }
        });
    }
}

impl fmt::Debug for Queues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_tuple("Queues");
        if let Some(queues) = self.queues.try_lock() {
            debug.field(&*queues);
        }
        debug.finish()
    }
}
