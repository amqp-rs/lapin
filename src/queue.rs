use crate::{
    options::QueueDeclareOptions,
    types::{ConsumerCount, MessageCount, ShortString},
};
use std::borrow::Borrow;

#[derive(Clone, Debug)]
pub struct Queue {
    name: ShortString,
    message_count: MessageCount,
    consumer_count: ConsumerCount,
}

impl Queue {
    pub(crate) fn new(
        name: ShortString,
        message_count: MessageCount,
        consumer_count: ConsumerCount,
    ) -> Self {
        Self {
            name,
            message_count,
            consumer_count,
        }
    }

    pub fn name(&self) -> &ShortString {
        &self.name
    }

    pub fn message_count(&self) -> MessageCount {
        self.message_count
    }

    pub fn consumer_count(&self) -> ConsumerCount {
        self.consumer_count
    }
}

impl Borrow<str> for Queue {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}

impl QueueDeclareOptions {
    /// The traditional queue type, persisted on the AMQP server.
    pub fn durable() -> Self {
        Self {
            durable: true,
            ..Default::default()
        }
    }

    /// Only the current connection can consume messages.
    /// When enabling exclusive, you probably want to enable auto_delete too.
    pub fn exclusive() -> Self {
        Self {
            exclusive: true,
            ..Default::default()
        }
    }

    /// The queue will get automatically deleted when its last consumer stops.
    pub fn auto_delete(mut self) -> Self {
        self.auto_delete = true;
        self
    }

    /// When enabling passive, we only checks for a queue existence.
    /// If the queue exists, the queue_declare call will succeed, otherwise a channel error is raised.
    pub fn passive(mut self) -> Self {
        self.passive = true;
        self
    }
}
