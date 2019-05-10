use log::error;
use parking_lot::Mutex;

use std::{
  collections::VecDeque,
  sync::Arc,
};

use crate::{
  channel::BasicProperties,
  message::BasicReturnMessage,
};

#[derive(Clone, Debug, Default)]
pub struct ReturnedMessages {
  inner: Arc<Mutex<Inner>>,
}

impl ReturnedMessages {
  pub fn start_new_delivery(&self, message: BasicReturnMessage) {
    self.inner.lock().current_message = Some(message);
  }

  pub fn set_delivery_properties(&self, properties: BasicProperties) {
    if let Some(message) = self.inner.lock().current_message.as_mut() {
      message.delivery.properties = properties;
    }
  }

  pub fn new_delivery_complete(&self) {
    self.inner.lock().new_delivery_complete();
  }

  pub fn receive_delivery_content(&self, data: Vec<u8>) {
    if let Some(message) = self.inner.lock().current_message.as_mut() {
      message.delivery.data.extend(data);
    }
  }
}

#[derive(Debug, Default)]
pub struct Inner {
  current_message: Option<BasicReturnMessage>,
  messages:        VecDeque<BasicReturnMessage>,
}

impl Inner {
  fn new_delivery_complete(&mut self) {
    if let Some(message) = self.current_message.take() {
      // FIXME, once there's a way to consume it: self.messages.push_back(message);
      error!("Server returned us a message: {:?}", message);
    }
  }
}
