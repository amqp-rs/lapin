use crate::{
    consumer::Consumer,
    message::Delivery,
    topology_internal::ConsumerDefinitionInternal,
    types::{LongLongUInt, ShortString},
    BasicProperties, Channel, Error, Result,
};
use parking_lot::Mutex;
use std::{borrow::Borrow, collections::HashMap, fmt, hash::Hash, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct Consumers(Arc<Mutex<HashMap<ShortString, Consumer>>>);

impl Consumers {
    pub(crate) fn register(&self, tag: ShortString, consumer: Consumer) {
        self.0.lock().insert(tag, consumer);
    }

    pub(crate) fn deregister<S: Hash + Eq + ?Sized>(&self, consumer_tag: &S) -> Result<()>
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.0.lock().remove(consumer_tag) {
            consumer.cancel()?;
        }
        Ok(())
    }

    pub(crate) fn start_cancel<S: Hash + Eq + ?Sized>(&self, consumer_tag: &S)
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.0.lock().get(consumer_tag) {
            consumer.start_cancel();
        }
    }

    pub(crate) fn start_delivery<S: Hash + Eq + ?Sized>(&self, consumer_tag: &S, message: Delivery)
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.0.lock().get_mut(consumer_tag) {
            consumer.start_new_delivery(message);
        }
    }

    pub(crate) fn handle_content_header_frame<S: Hash + Eq + ?Sized>(
        &self,
        channel: &Channel,
        consumer_tag: &S,
        size: LongLongUInt,
        properties: BasicProperties,
    ) -> Result<()>
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.0.lock().get_mut(consumer_tag) {
            consumer.handle_content_header_frame(channel, size, properties)?;
        }
        Ok(())
    }

    pub(crate) fn handle_body_frame<S: Hash + Eq + ?Sized>(
        &self,
        channel: &Channel,
        consumer_tag: &S,
        remaining_size: LongLongUInt,
        payload: Vec<u8>,
    ) -> Result<()>
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.0.lock().get_mut(consumer_tag) {
            consumer.handle_body_frame(channel, remaining_size, payload)?;
        }
        Ok(())
    }

    pub(crate) fn drop_prefetched_messages(&self) -> Result<()> {
        self.0
            .lock()
            .values()
            .map(Consumer::drop_prefetched_messages)
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn cancel(&self) -> Result<()> {
        self.0
            .lock()
            .drain()
            .map(|(_, consumer)| consumer.cancel())
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn error(&self, error: Error) -> Result<()> {
        self.0
            .lock()
            .drain()
            .map(|(_, consumer)| consumer.set_error(error.clone()))
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn topology(&self) -> Vec<ConsumerDefinitionInternal> {
        self.0
            .lock()
            .values()
            .map(|consumer| ConsumerDefinitionInternal::new(consumer.clone()))
            .collect()
    }
}

impl fmt::Debug for Consumers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_tuple("Consumers");
        if let Some(consumers) = self.0.try_lock() {
            debug.field(&*consumers);
        }
        debug.finish()
    }
}
