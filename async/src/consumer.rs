use std::fmt::Debug;

use crate::message::Delivery;

#[derive(Debug)]
pub struct Consumer {
      tag:             String,
      no_local:        bool,
      no_ack:          bool,
      exclusive:       bool,
      nowait:          bool,
      subscriber:      Box<dyn ConsumerSubscriber>,
  pub current_message: Option<Delivery>,
}

impl Consumer {
  pub fn new(tag: String, no_local: bool, no_ack: bool, exclusive: bool, nowait: bool, subscriber: Box<dyn ConsumerSubscriber>) -> Consumer {
    Consumer {
      tag,
      no_local,
      no_ack,
      exclusive,
      nowait,
      subscriber,
      current_message: None,
    }
  }

  pub fn new_delivery_complete(&mut self) {
    if let Some(delivery) = self.current_message.take() {
      self.subscriber.new_delivery(delivery);
    }
  }

  pub fn drop_prefetched_messages(&mut self) {
    self.subscriber.drop_prefetched_messages();
  }
}

pub trait ConsumerSubscriber: Debug+Send+Sync {
  fn new_delivery(&mut self, delivery: Delivery);
  fn drop_prefetched_messages(&mut self);
}
