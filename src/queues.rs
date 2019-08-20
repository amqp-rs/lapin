use parking_lot::Mutex;

use std::{collections::HashMap, sync::Arc};

use crate::{
    consumer::Consumer,
    message::{BasicGetMessage, Delivery},
    queue::QueueState,
    types::ShortString,
    wait::WaitHandle,
    BasicProperties,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct Queues {
    queues: Arc<Mutex<HashMap<ShortString, QueueState>>>,
}

impl Queues {
    pub(crate) fn register(&self, queue: QueueState) {
        self.queues.lock().insert(queue.name(), queue);
    }

    pub(crate) fn deregister(&self, queue: &str) {
        self.queues.lock().remove(queue);
    }

    pub(crate) fn register_consumer(
        &self,
        queue: &str,
        consumer_tag: ShortString,
        consumer: Consumer,
    ) {
        if let Some(queue) = self.queues.lock().get_mut(queue) {
            queue.register_consumer(consumer_tag, consumer);
        }
    }

    pub(crate) fn deregister_consumer(&self, consumer_tag: &str) {
        for queue in self.queues.lock().values_mut() {
            queue.deregister_consumer(consumer_tag);
        }
    }

    pub(crate) fn drop_prefetched_messages(&self) {
        for queue in self.queues.lock().values_mut() {
            queue.drop_prefetched_messages();
        }
    }

    pub(crate) fn cancel_consumers(&self) {
        for queue in self.queues.lock().values_mut() {
            queue.cancel_consumers();
        }
    }

    pub(crate) fn error_consumers(&self) {
        for queue in self.queues.lock().values_mut() {
            queue.error_consumers();
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
        wait_handle: WaitHandle<Option<BasicGetMessage>>,
    ) {
        if let Some(queue) = self.queues.lock().get_mut(queue) {
            queue.start_new_delivery(message, wait_handle);
        }
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        queue: &str,
        consumer_tag: Option<ShortString>,
        size: u64,
        properties: BasicProperties,
    ) {
        if let Some(queue) = self.queues.lock().get_mut(queue) {
            match consumer_tag {
                Some(consumer_tag) => {
                    if let Some(consumer) = queue.get_consumer(&consumer_tag) {
                        consumer.set_delivery_properties(properties);
                        if size == 0 {
                            consumer.new_delivery_complete();
                        }
                    }
                }
                None => {
                    queue.set_delivery_properties(properties);
                    if size == 0 {
                        queue.new_delivery_complete();
                    }
                }
            }
        }
    }

    pub(crate) fn handle_body_frame(
        &self,
        queue: &str,
        consumer_tag: Option<ShortString>,
        remaining_size: usize,
        payload_size: usize,
        payload: Vec<u8>,
    ) {
        if let Some(queue) = self.queues.lock().get_mut(queue) {
            match consumer_tag {
                Some(consumer_tag) => {
                    if let Some(consumer) = queue.get_consumer(&consumer_tag) {
                        consumer.receive_delivery_content(payload);
                        if remaining_size == payload_size {
                            consumer.new_delivery_complete();
                        }
                    }
                }
                None => {
                    queue.receive_delivery_content(payload);
                    if remaining_size == payload_size {
                        queue.new_delivery_complete();
                    }
                }
            }
        }
    }
}
