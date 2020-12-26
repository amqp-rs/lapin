use crate::types::{ConsumerCount, MessageCount, ShortString};
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
