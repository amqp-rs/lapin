#[derive(Debug, Clone)]
pub struct Queue {
  consumer_count: u32,
  message_count:  u32,
  name:           String,
}

impl Queue {
  pub fn new(name: String, consumer_count: u32, message_count: u32) -> Self {
    Self {
      consumer_count,
      message_count,
      name,
    }
  }

  pub fn name(&self) -> String {
    self.name.clone()
  }

  pub fn consumer_count(&self) -> u32 {
    self.consumer_count
  }

  pub fn message_count(&self) -> u32 {
    self.message_count
  }
}
