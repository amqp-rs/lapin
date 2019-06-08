use parking_lot::Mutex;

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use crate::{
  error::{Error, ErrorKind},
  types::Boolean,
  wait::{Wait, WaitHandle},
};

pub type DeliveryTag = u64;

#[derive(Debug, Default, Clone)]
pub struct Acknowledgements {
  inner: Arc<Mutex<Inner>>,
}

impl Acknowledgements {
  pub fn register_pending(&self, delivery_tag: DeliveryTag) {
    self.inner.lock().register_pending(delivery_tag);
  }

  pub fn get_last_pending(&self) -> Option<Wait<Option<Boolean>>> {
    self.inner.lock().last.take()
  }

  pub fn ack(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.inner.lock().ack(delivery_tag)
  }

  pub fn nack(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.inner.lock().nack(delivery_tag)
  }

  pub fn ack_all_pending(&self) {
    let mut inner = self.inner.lock();
    for wait in inner.drain_pending() {
      wait.finish(Some(true));
    }
  }

  pub fn nack_all_pending(&self) {
    let mut inner = self.inner.lock();
    for wait in inner.drain_pending() {
      wait.finish(Some(false));
    }
  }

  pub fn ack_all_before(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    let mut inner = self.inner.lock();
    for tag in inner.list_pending_before(delivery_tag) {
      inner.ack(tag)?;
    }
    Ok(())
  }

  pub fn nack_all_before(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    let mut inner = self.inner.lock();
    for tag in inner.list_pending_before(delivery_tag) {
      inner.nack(tag)?;
    }
    Ok(())
  }
}

#[derive(Debug, Default)]
struct Inner {
  last:    Option<Wait<Option<Boolean>>>,
  pending: HashMap<DeliveryTag, WaitHandle<Option<Boolean>>>,
}

impl Inner {
  fn register_pending(&mut self, delivery_tag: DeliveryTag) {
    let (wait, wait_handle) = Wait::new();
    self.pending.insert(delivery_tag, wait_handle);
    self.last = Some(wait);
  }

  fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: Boolean) -> Result<(), Error> {
    if let Some(delivery_wait) =  self.pending.remove(&delivery_tag) {
      delivery_wait.finish(Some(success));
      Ok(())
    } else {
      Err(ErrorKind::PreconditionFailed.into())
    }
  }

  fn ack(&mut self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.drop_pending(delivery_tag, true)?;
    Ok(())
  }

  fn nack(&mut self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.drop_pending(delivery_tag, false)?;
    Ok(())
  }

  fn drain_pending(&mut self) -> Vec<WaitHandle<Option<Boolean>>> {
    self.pending.drain().map(|tup| tup.1).collect()
  }

  fn list_pending_before(&mut self, delivery_tag: DeliveryTag) -> HashSet<DeliveryTag> {
    self.pending.iter().map(|tup| tup.0).filter(|tag| **tag <= delivery_tag).cloned().collect()
  }
}
