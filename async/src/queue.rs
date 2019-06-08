use std::collections::HashMap;

use crate::{
  channel::BasicProperties,
  consumer::Consumer,
  message::BasicGetMessage,
  types::ShortString,
  wait::WaitHandle,
};

#[derive(Clone, Debug)]
pub struct Queue {
  pub name:           ShortString,
  pub message_count:  u32,
  pub consumer_count: u32,
}

#[derive(Debug)]
pub struct QueueState {
  pub name:                ShortString,
  pub consumers:           HashMap<ShortString, Consumer>,
      current_get_message: Option<(BasicGetMessage, WaitHandle<Option<BasicGetMessage>>)>,
}

impl Queue {
  pub fn new(name: ShortString, message_count: u32, consumer_count: u32) -> Self {
    Self { name, message_count, consumer_count }
  }
}

impl QueueState {
  pub fn drop_prefetched_messages(&mut self) {
    for consumer in self.consumers.values() {
      consumer.drop_prefetched_messages();
    }
  }

  pub fn start_new_delivery(&mut self, delivery: BasicGetMessage, wait_handle: WaitHandle<Option<BasicGetMessage>>) {
    self.current_get_message = Some((delivery, wait_handle));
  }

  pub fn set_delivery_properties(&mut self, properties: BasicProperties) {
    if let Some(delivery) = self.current_get_message.as_mut() {
      delivery.0.delivery.properties = properties;
    }
  }

  pub fn receive_delivery_content(&mut self, payload: Vec<u8>) {
    if let Some(delivery) = self.current_get_message.as_mut() {
      delivery.0.delivery.receive_content(payload);
    }
  }

  pub fn new_delivery_complete(&mut self) {
    if let Some((message, wait_handle)) = self.current_get_message.take() {
      wait_handle.finish(Some(message));
    }
  }
}

impl From<Queue> for QueueState {
  fn from(queue: Queue) -> Self {
    Self {
      name:                queue.name,
      consumers:           HashMap::new(),
      current_get_message: None,
    }
  }
}
