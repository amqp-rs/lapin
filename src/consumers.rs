use crate::{
    BasicProperties, Error,
    consumer::Consumer,
    error_holder::ErrorHolder,
    message::Delivery,
    types::{PayloadSize, ShortString},
};
use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt,
    hash::Hash,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

type Inner = HashMap<ShortString, Consumer>;

#[derive(Clone, Default)]
pub(crate) struct Consumers(Arc<RwLock<Inner>>);

impl Consumers {
    pub(crate) fn register(&self, tag: ShortString, consumer: Consumer) {
        self.write().insert(tag, consumer);
    }

    pub(crate) fn deregister<S: Hash + Eq + ?Sized>(&self, consumer_tag: &S)
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.write().remove(consumer_tag) {
            consumer.cancel();
        }
    }

    pub(crate) fn start_cancel_one<S: Hash + Eq + ?Sized>(&self, consumer_tag: &S)
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.read().get(consumer_tag) {
            consumer.start_cancel();
        }
    }

    pub(crate) fn start_delivery<S: Hash + Eq + ?Sized, F: FnOnce(ErrorHolder) -> Delivery>(
        &self,
        consumer_tag: &S,
        message: F,
    ) where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.read().get(consumer_tag) {
            consumer.start_new_delivery(message(consumer.error()));
        }
    }

    pub(crate) fn handle_content_header_frame<S: Hash + Eq + ?Sized>(
        &self,
        consumer_tag: &S,
        size: PayloadSize,
        properties: BasicProperties,
    ) where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.read().get(consumer_tag) {
            consumer.handle_content_header_frame(size, properties);
        }
    }

    pub(crate) fn handle_body_frame<S: Hash + Eq + ?Sized>(
        &self,
        consumer_tag: &S,
        remaining_size: PayloadSize,
        payload: Vec<u8>,
    ) where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.read().get(consumer_tag) {
            consumer.handle_body_frame(remaining_size, payload);
        }
    }

    pub(crate) fn drop_prefetched_messages(&self) {
        for consumer in self.read().values() {
            consumer.drop_prefetched_messages();
        }
    }

    pub(crate) fn start_cancel(&self) {
        for consumer in self.read().values() {
            consumer.start_cancel();
        }
    }

    pub(crate) fn cancel(&self) {
        for (_, consumer) in self.write().drain() {
            consumer.cancel();
        }
    }

    pub(crate) fn error(&self, error: Error, recover: bool) {
        for (_, consumer) in self.write().drain() {
            if recover {
                consumer.send_error(error.clone());
            } else {
                consumer.set_error(error.clone());
            }
        }
    }

    pub(crate) fn topology(&self) -> Vec<Consumer> {
        self.read().values().cloned().collect()
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.0.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.0.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for Consumers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_tuple("Consumers");
        if let Ok(consumers) = self.0.try_read() {
            debug.field(&*consumers);
        }
        debug.finish()
    }
}
