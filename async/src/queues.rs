use either::Either;
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
  requests::RequestId,
  types::ShortString,
};

#[derive(Clone, Debug, Default)]
pub struct Queues {
  queues: Arc<Mutex<HashMap<ShortString, Queue>>>,
}

impl Queues {
  pub fn register(&self, queue: Queue) {
    self.queues.lock().insert(queue.name.clone(), queue);
  }

  pub fn deregister(&self, queue: &str) {
    self.queues.lock().remove(queue);
  }

  pub fn register_consumer(&self, queue: &str, consumer_tag: ShortString, consumer: Consumer) {
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

  pub fn get_basic_get_message(&self, queue: &str, request_id: RequestId) -> Option<BasicGetMessage> {
    self.queues.lock().get_mut(queue).and_then(|queue| queue.get_basic_get_message(request_id))
  }

  pub fn start_consumer_delivery(&self, consumer_tag: &str, message: Delivery) -> Option<ShortString> {
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

  pub fn handle_content_header_frame(&self, queue: &str, request_id_or_consumer_tag: Either<RequestId, ShortString>, size: u64, properties: BasicProperties) {
    if let Some(queue) = self.queues.lock().get_mut(queue) {
      match request_id_or_consumer_tag {
        Either::Right(consumer_tag) => {
          if let Some(consumer) = queue.consumers.get_mut(&consumer_tag) {
            consumer.set_delivery_properties(properties);
            if size == 0 {
              consumer.new_delivery_complete();
            }
          }
        },
        Either::Left(request_id) => {
          queue.set_delivery_properties(properties);
          if size == 0 {
            queue.new_delivery_complete(request_id);
          }
        },
      }
    }
  }

  pub fn handle_body_frame(&self, queue: &str, request_id_or_consumer_tag: Either<RequestId, ShortString>, remaining_size: usize, payload_size: usize, payload: Vec<u8>) {
    if let Some(queue) = self.queues.lock().get_mut(queue) {
      match request_id_or_consumer_tag {
        Either::Right(consumer_tag) => {
          if let Some(consumer) = queue.consumers.get_mut(&consumer_tag) {
            consumer.receive_delivery_content(payload);
            if remaining_size == payload_size {
              consumer.new_delivery_complete();
            }
          }
        },
        Either::Left(request_id) => {
          queue.receive_delivery_content(payload);
          if remaining_size == payload_size {
            queue.new_delivery_complete(request_id);
          }
        },
      }
    }
  }
}
