use log::error;
use parking_lot::Mutex;

use std::sync::Arc;

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

  pub fn drain(&self) -> Vec<BasicReturnMessage> {
    self.inner.lock().messages.drain(..).collect()
  }
}

#[derive(Debug, Default)]
pub struct Inner {
  current_message: Option<BasicReturnMessage>,
  messages:        Vec<BasicReturnMessage>,
}

impl Inner {
  fn new_delivery_complete(&mut self) {
    if let Some(message) = self.current_message.take() {
      error!("Server returned us a message: {:?}", message);
      self.messages.push(message);
    }
  }
}
