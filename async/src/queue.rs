use std::collections::HashMap;

use crate::channel::BasicProperties;
use crate::consumer::Consumer;
use crate::message::BasicGetMessage;

#[derive(Debug)]
pub struct Queue {
  pub name:                String,
  pub consumers:           HashMap<String, Consumer>,
  pub stats:               QueueStats,
      get_message:         Option<BasicGetMessage>,
      current_get_message: Option<BasicGetMessage>,
}

#[derive(Clone, Debug, Default)]
pub struct QueueStats {
  pub message_count:  u32,
  pub consumer_count: u32,
}

impl Queue {
  pub fn new(name: String, message_count: u32, consumer_count: u32) -> Queue {
    Queue {
      name,
      consumers:           HashMap::new(),
      stats:               QueueStats { message_count, consumer_count },
      get_message:         None,
      current_get_message: None,
    }
  }

  pub fn next_basic_get_message(&mut self) -> Option<BasicGetMessage> {
    self.get_message.take()
  }

  pub fn drop_prefetched_messages(&mut self) {
    self.next_basic_get_message();
    for consumer in self.consumers.values() {
      consumer.drop_prefetched_messages();
    }
  }

  pub fn start_new_delivery(&mut self, delivery: BasicGetMessage) {
    self.current_get_message = Some(delivery)
  }

  pub fn set_delivery_properties(&mut self, properties: BasicProperties) {
    if let Some(delivery) = self.current_get_message.as_mut() {
      delivery.delivery.properties = properties;
    }
  }

  pub fn receive_delivery_content(&mut self, payload: Vec<u8>) {
    if let Some(delivery) = self.current_get_message.as_mut() {
      delivery.delivery.receive_content(payload);
    }
  }

  pub fn new_delivery_complete(&mut self) {
    self.get_message = self.current_get_message.take();
  }
}
