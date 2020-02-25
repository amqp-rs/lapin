use crate::{
    consumer::Consumer, message::BasicGetMessage, pinky_swear::Pinky, types::ShortString,
    BasicProperties, Error, Result,
};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

#[derive(Clone, Debug)]
pub struct Queue {
    name: ShortString,
    message_count: u32,
    consumer_count: u32,
}

impl Queue {
    pub fn name(&self) -> &ShortString {
        &self.name
    }

    pub fn message_count(&self) -> u32 {
        self.message_count
    }

    pub fn consumer_count(&self) -> u32 {
        self.consumer_count
    }
}

#[derive(Debug)]
pub(crate) struct QueueState {
    name: ShortString,
    consumers: HashMap<ShortString, Consumer>,
    current_get_message: Option<(BasicGetMessage, Pinky<Result<Option<BasicGetMessage>>>)>,
}

impl Queue {
    pub(crate) fn new(name: ShortString, message_count: u32, consumer_count: u32) -> Self {
        Self {
            name,
            message_count,
            consumer_count,
        }
    }
}

impl Borrow<str> for Queue {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}

impl QueueState {
    pub(crate) fn register_consumer(&mut self, consumer_tag: ShortString, consumer: Consumer) {
        self.consumers.insert(consumer_tag, consumer);
    }

    pub(crate) fn deregister_consumer<S: Hash + Eq + ?Sized>(
        &mut self,
        consumer_tag: &S,
    ) -> Result<()>
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.consumers.remove(consumer_tag) {
            consumer.cancel()?;
        }
        Ok(())
    }

    pub(crate) fn get_consumer<S: Hash + Eq + ?Sized>(
        &mut self,
        consumer_tag: &S,
    ) -> Option<&mut Consumer>
    where
        ShortString: Borrow<S>,
    {
        self.consumers.get_mut(consumer_tag.borrow())
    }

    pub(crate) fn cancel_consumers(&mut self) -> Result<()> {
        self.consumers
            .drain()
            .map(|(_, consumer)| consumer.cancel())
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn error_consumers(&mut self, error: Error) -> Result<()> {
        self.consumers
            .drain()
            .map(|(_, consumer)| consumer.set_error(error.clone()))
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn name(&self) -> ShortString {
        self.name.clone()
    }

    pub(crate) fn drop_prefetched_messages(&mut self) -> Result<()> {
        self.consumers
            .values()
            .map(Consumer::drop_prefetched_messages)
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn start_new_delivery(
        &mut self,
        delivery: BasicGetMessage,
        pinky: Pinky<Result<Option<BasicGetMessage>>>,
    ) {
        self.current_get_message = Some((delivery, pinky));
    }

    pub(crate) fn set_delivery_properties(&mut self, properties: BasicProperties) {
        if let Some(delivery) = self.current_get_message.as_mut() {
            delivery.0.delivery.properties = properties;
        }
    }

    pub(crate) fn receive_delivery_content(&mut self, payload: Vec<u8>) {
        if let Some(delivery) = self.current_get_message.as_mut() {
            delivery.0.delivery.receive_content(payload);
        }
    }

    pub(crate) fn new_delivery_complete(&mut self) {
        if let Some((message, pinky)) = self.current_get_message.take() {
            pinky.swear(Ok(Some(message)));
        }
    }
}

impl From<Queue> for QueueState {
    fn from(queue: Queue) -> Self {
        Self {
            name: queue.name,
            consumers: HashMap::new(),
            current_get_message: None,
        }
    }
}
