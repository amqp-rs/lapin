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

    #[must_use]
    pub fn name(&self) -> &ShortString {
        &self.name
    }

    #[must_use]
    pub fn message_count(&self) -> MessageCount {
        self.message_count
    }

    #[must_use]
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
    #[must_use]
    pub fn durable() -> Self {
        Self {
            durable: true,
            ..Default::default()
        }
    }

    /// Only the current connection can consume messages.
    /// When enabling exclusive, you probably want to enable auto_delete too.
    #[must_use]
    pub fn exclusive() -> Self {
        Self {
            exclusive: true,
            ..Default::default()
        }
    }

    /// The queue will get automatically deleted when its last consumer stops.
    #[must_use]
    pub fn auto_delete(mut self) -> Self {
        self.auto_delete = true;
        self
    }

    /// When enabling passive, we only checks for a queue existence.
    /// If the queue exists, the queue_declare call will succeed, otherwise a channel error is raised.
    #[must_use]
    pub fn passive(mut self) -> Self {
        self.passive = true;
        self
    }
}
