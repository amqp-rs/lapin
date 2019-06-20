use log::error;
use parking_lot::Mutex;

use std::{
  collections::VecDeque,
  sync::Arc,
};

use crate::{
  BasicProperties,
  message::BasicReturnMessage,
  wait::WaitHandle,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct ReturnedMessages {
  inner: Arc<Mutex<Inner>>,
}

impl ReturnedMessages {
  pub(crate) fn start_new_delivery(&self, message: BasicReturnMessage) {
    self.inner.lock().current_message = Some(message);
  }

  pub(crate) fn set_delivery_properties(&self, properties: BasicProperties) {
    if let Some(message) = self.inner.lock().current_message.as_mut() {
      message.delivery.properties = properties;
    }
  }

  pub(crate) fn new_delivery_complete(&self) {
    self.inner.lock().new_delivery_complete();
  }

  pub(crate) fn receive_delivery_content(&self, data: Vec<u8>) {
    if let Some(message) = self.inner.lock().current_message.as_mut() {
      message.delivery.data.extend(data);
    }
  }

  pub(crate) fn drain(&self) -> Vec<BasicReturnMessage> {
    self.inner.lock().messages.drain(..).collect()
  }

  pub(crate) fn register_waiter(&self, waiter: WaitHandle<()>) {
    self.inner.lock().waiters.push_back(waiter);
  }
}

#[derive(Debug, Default)]
pub struct Inner {
  current_message: Option<BasicReturnMessage>,
  messages:        Vec<BasicReturnMessage>,
  waiters:         VecDeque<WaitHandle<()>>,
}

impl Inner {
  fn new_delivery_complete(&mut self) {
    if let Some(message) = self.current_message.take() {
      error!("Server returned us a message: {:?}", message);
      self.messages.push(message);
      if let Some(wait_handle) = self.waiters.pop_front() {
        wait_handle.finish(());
      }
    }
  }
}
