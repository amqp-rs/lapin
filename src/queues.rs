use crate::{
    consumer::Consumer,
    message::{BasicGetMessage, Delivery},
    queue::{Queue, QueueState},
    types::ShortString,
    BasicProperties, Error, PromiseResolver, Result,
};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Clone, Default)]
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

    fn with_queue<F: FnOnce(&mut QueueState) -> Result<()>>(
        &self,
        queue: &str,
        f: F,
    ) -> Result<()> {
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
            Ok(())
        })
        .expect("register_consumer cannot fail");
    }

    pub(crate) fn deregister_consumer(&self, consumer_tag: &str) -> Result<()> {
        self.queues
            .lock()
            .values_mut()
            .map(|queue| queue.deregister_consumer(consumer_tag))
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn drop_prefetched_messages(&self) -> Result<()> {
        self.queues
            .lock()
            .values_mut()
            .map(QueueState::drop_prefetched_messages)
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn cancel_consumers(&self) -> Result<()> {
        self.queues
            .lock()
            .values_mut()
            .map(QueueState::cancel_consumers)
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn error_consumers(&self, error: Error) -> Result<()> {
        self.queues
            .lock()
            .values_mut()
            .map(|queue| queue.error_consumers(error.clone()))
            .fold(Ok(()), Result::and)
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
            Ok(())
        })
        .expect("start_basic_get_delivery cannot fail");
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        queue: &str,
        consumer_tag: Option<ShortString>,
        size: u64,
        properties: BasicProperties,
    ) -> Result<()> {
        self.with_queue(queue, |queue| {
            match consumer_tag {
                Some(consumer_tag) => {
                    if let Some(consumer) = queue.get_consumer(&consumer_tag) {
                        consumer.set_delivery_properties(properties);
                        if size == 0 {
                            consumer.new_delivery_complete()?;
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
            Ok(())
        })
    }

    pub(crate) fn handle_body_frame(
        &self,
        queue: &str,
        consumer_tag: Option<ShortString>,
        remaining_size: usize,
        payload: Vec<u8>,
    ) -> Result<()> {
        self.with_queue(queue, |queue| {
            match consumer_tag {
                Some(consumer_tag) => {
                    if let Some(consumer) = queue.get_consumer(&consumer_tag) {
                        consumer.receive_delivery_content(payload);
                        if remaining_size == 0 {
                            consumer.new_delivery_complete()?;
                        }
                    }
                }
                None => {
                    queue.receive_delivery_content(payload);
                    if remaining_size == 0 {
                        queue.new_delivery_complete();
                    }
                }
            }
            Ok(())
        })
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
