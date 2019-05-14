use crate::types::ShortString;

#[derive(Debug, Clone)]
pub struct Queue {
  consumer_count: u32,
  message_count:  u32,
  name:           ShortString,
}

impl Queue {
  pub fn new(name: ShortString, consumer_count: u32, message_count: u32) -> Self {
    Self {
      consumer_count,
      message_count,
      name,
    }
  }

  pub fn name(&self) -> ShortString {
    self.name.clone()
  }

  pub fn consumer_count(&self) -> u32 {
    self.consumer_count
  }

  pub fn message_count(&self) -> u32 {
    self.message_count
  }
}
