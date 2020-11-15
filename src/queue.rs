use crate::types::ShortString;
use std::borrow::Borrow;

#[derive(Clone, Debug)]
pub struct Queue {
    name: ShortString,
    message_count: u32,
    consumer_count: u32,
}

impl Queue {
    pub(crate) fn new(name: ShortString, message_count: u32, consumer_count: u32) -> Self {
        Self {
            name,
            message_count,
            consumer_count,
        }
    }

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

impl Borrow<str> for Queue {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}
