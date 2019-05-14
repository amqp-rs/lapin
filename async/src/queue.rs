use std::collections::HashMap;

use crate::{
  channel::BasicProperties,
  consumer::Consumer,
  message::BasicGetMessage,
  requests::RequestId,
  types::ShortString,
};

#[derive(Debug)]
pub struct Queue {
  pub name:                ShortString,
  pub consumers:           HashMap<ShortString, Consumer>,
  pub stats:               QueueStats,
      get_messages:        HashMap<RequestId, BasicGetMessage>,
      current_get_message: Option<BasicGetMessage>,
}

#[derive(Clone, Debug, Default)]
pub struct QueueStats {
  pub message_count:  u32,
  pub consumer_count: u32,
}

impl Queue {
  pub fn new(name: ShortString, message_count: u32, consumer_count: u32) -> Queue {
    Queue {
      name,
      consumers:           HashMap::new(),
      stats:               QueueStats { message_count, consumer_count },
      get_messages:        HashMap::new(),
      current_get_message: None,
    }
  }

  pub fn get_basic_get_message(&mut self, request_id: RequestId) -> Option<BasicGetMessage> {
    self.get_messages.remove(&request_id)
  }

  pub fn drop_prefetched_messages(&mut self) {
    self.get_messages.clear();
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

  pub fn new_delivery_complete(&mut self, request_id: RequestId) {
    if let Some(message) = self.current_get_message.take() {
      self.get_messages.insert(request_id, message);
    }
  }
}
