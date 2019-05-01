use parking_lot::Mutex;

use std::{
  collections::HashSet,
  sync::Arc,
};

use crate::error::{Error, ErrorKind};

pub type DeliveryTag = u64;

#[derive(Debug, Default, Clone)]
pub struct Acknowledgements {
  inner: Arc<Mutex<Inner>>,
}

impl Acknowledgements {
  pub fn register_pending(&self, delivery_tag: DeliveryTag) {
    self.inner.lock().pending.insert(delivery_tag);
  }

  pub fn ack(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.inner.lock().ack(delivery_tag)
  }

  pub fn nack(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.inner.lock().nack(delivery_tag)
  }

  pub fn ack_all_pending(&self) {
    let mut inner = self.inner.lock();
    for tag in inner.drain_pending() {
      inner.acked.insert(tag);
    }
  }

  pub fn nack_all_pending(&self) {
    let mut inner = self.inner.lock();
    for tag in inner.drain_pending() {
      inner.nacked.insert(tag);
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

  pub fn is_acked(&self, delivery_tag: DeliveryTag) -> bool {
    self.inner.lock().acked.remove(&delivery_tag)
  }

  pub fn is_nacked(&self, delivery_tag: DeliveryTag) -> bool {
    self.inner.lock().nacked.remove(&delivery_tag)
  }
}

#[derive(Debug, Default)]
struct Inner {
  pending: HashSet<DeliveryTag>,
  acked:   HashSet<DeliveryTag>,
  nacked:  HashSet<DeliveryTag>,
}

impl Inner {
  fn drop_pending(&mut self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    if self.pending.remove(&delivery_tag) {
      Ok(())
    } else {
      Err(ErrorKind::PreconditionFailed.into())
    }
  }

  fn ack(&mut self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.drop_pending(delivery_tag)?;
    self.acked.insert(delivery_tag);
    Ok(())
  }

  fn nack(&mut self, delivery_tag: DeliveryTag) -> Result<(), Error> {
    self.drop_pending(delivery_tag)?;
    self.nacked.insert(delivery_tag);
    Ok(())
  }

  fn drain_pending(&mut self) -> HashSet<DeliveryTag> {
    self.pending.drain().collect()
  }

  fn list_pending_before(&mut self, delivery_tag: DeliveryTag) -> HashSet<DeliveryTag> {
    self.pending.iter().filter(|tag| **tag <= delivery_tag).cloned().collect()
  }
}
