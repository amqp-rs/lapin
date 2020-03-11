use std::fmt::Debug;

use crate::{
  BasicProperties,
  message::Delivery,
  types::ShortString,
};

#[derive(Debug)]
pub(crate) struct Consumer {
  tag:             ShortString,
  no_local:        bool,
  no_ack:          bool,
  exclusive:       bool,
  subscriber:      Box<dyn ConsumerSubscriber>,
  current_message: Option<Delivery>,
}

impl Consumer {
  pub(crate) fn new(tag: ShortString, no_local: bool, no_ack: bool, exclusive: bool, subscriber: Box<dyn ConsumerSubscriber>) -> Consumer {
    Consumer {
      tag,
      no_local,
      no_ack,
      exclusive,
      subscriber,
      current_message: None,
    }
  }

  pub(crate) fn start_new_delivery(&mut self, delivery: Delivery) {
    self.current_message = Some(delivery)
  }

  pub(crate) fn set_delivery_properties(&mut self, properties: BasicProperties) {
    if let Some(delivery) = self.current_message.as_mut() {
      delivery.properties = properties;
    }
  }

  pub(crate) fn receive_delivery_content(&mut self, payload: Vec<u8>) {
    if let Some(delivery) = self.current_message.as_mut() {
      delivery.receive_content(payload);
    }
  }

  pub(crate) fn new_delivery_complete(&mut self) {
    if let Some(delivery) = self.current_message.take() {
      self.subscriber.new_delivery(delivery);
    }
  }

  pub(crate) fn drop_prefetched_messages(&self) {
    self.subscriber.drop_prefetched_messages();
  }

  pub(crate) fn cancel(&self) {
    self.subscriber.cancel();
  }
}

#[deprecated(note = "use lapin instead")]
pub trait ConsumerSubscriber: Debug + Send + Sync {
  fn new_delivery(&self, delivery: Delivery);
  fn drop_prefetched_messages(&self);
  fn cancel(&self);
}
