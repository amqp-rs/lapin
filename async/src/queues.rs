use parking_lot::Mutex;

use std::{
  collections::HashMap,
  sync::Arc,
};

use crate::{
  channel::BasicProperties,
  consumer::Consumer,
  queue::{Queue, QueueStats},
  message::{BasicGetMessage, Delivery},
};

#[derive(Clone, Debug, Default)]
pub struct Queues {
  queues: Arc<Mutex<HashMap<String, Queue>>>,
}

impl Queues {
  pub fn register(&self, queue: Queue) {
    self.queues.lock().insert(queue.name.clone(), queue);
  }

  pub fn deregister(&self, queue: &str) {
    self.queues.lock().remove(queue);
  }

  pub fn register_consumer(&self, queue: &str, consumer_tag: String, consumer: Consumer) {
    if let Some(queue) = self.queues.lock().get_mut(queue) {
      queue.consumers.insert(consumer_tag, consumer);
    }
  }

  pub fn deregister_consumer(&self, consumer_tag: &str) {
    for queue in self.queues.lock().values_mut() {
      if let Some(consumer) = queue.consumers.remove(consumer_tag) {
        consumer.cancel();
      }
    }
  }

  pub fn drop_prefetched_messages(&self) {
    for queue in self.queues.lock().values_mut() {
      queue.drop_prefetched_messages();
    }
  }

  pub fn get_stats(&self, queue: &str) -> QueueStats {
    self.queues.lock().get(queue).map(|queue| queue.stats.clone()).unwrap_or_default()
  }

  pub fn next_basic_get_message(&self, queue: &str) -> Option<BasicGetMessage> {
    self.queues.lock().get_mut(queue).and_then(Queue::next_basic_get_message)
  }

  pub fn start_consumer_delivery(&self, consumer_tag: &str, message: Delivery) -> Option<String> {
    for queue in self.queues.lock().values_mut() {
      if let Some(consumer) = queue.consumers.get_mut(consumer_tag) {
        consumer.start_new_delivery(message);
        return Some(queue.name.clone());
      }
    }
    None
  }

  pub fn start_basic_get_delivery(&self, queue: &str, message: BasicGetMessage) {
    if let Some(queue) = self.queues.lock().get_mut(queue) {
      queue.start_new_delivery(message);
    }
  }

  pub fn handle_content_header_frame(&self, queue: &str, consumer_tag: Option<String>, size: u64, properties: BasicProperties) {
    if let Some(queue) = self.queues.lock().get_mut(queue) {
      if let Some(consumer_tag) = consumer_tag {
        if let Some(consumer) = queue.consumers.get_mut(&consumer_tag) {
          consumer.set_delivery_properties(properties);
          if size == 0 {
            consumer.new_delivery_complete();
          }
        }
      } else {
        queue.set_delivery_properties(properties);
        if size == 0 {
          queue.new_delivery_complete();
        }
      }
    }
  }

  pub fn handle_body_frame(&self, queue: &str, consumer_tag: Option<String>, remaining_size: usize, payload_size: usize, payload: Vec<u8>) {
    if let Some(queue) = self.queues.lock().get_mut(queue) {
      if let Some(consumer_tag) = consumer_tag {
        if let Some(consumer) = queue.consumers.get_mut(&consumer_tag) {
          consumer.receive_delivery_content(payload);
          if remaining_size == payload_size {
            consumer.new_delivery_complete();
          }
        }
      } else {
        queue.receive_delivery_content(payload);
        if remaining_size == payload_size {
          queue.new_delivery_complete();
        }
      }
    }
  }
}
