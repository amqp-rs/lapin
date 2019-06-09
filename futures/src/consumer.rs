use futures::{Async, Poll, Stream, task};
use log::trace;
use parking_lot::Mutex;

use std::{
  collections::VecDeque,
  sync::Arc,
};

use crate::{
  ConsumerSubscriber, Error,
  message::Delivery,
  types::ShortString,
};

#[derive(Clone, Debug, Default)]
pub struct Consumer {
  inner: Arc<Mutex<ConsumerInner>>,
}

impl Consumer {
  pub(crate) fn set_consumer_tag(&self, consumer_tag: ShortString) {
    self.inner.lock().tag = consumer_tag;
  }
}

#[derive(Debug)]
struct ConsumerInner {
  deliveries: VecDeque<Delivery>,
  task:       Option<task::Task>,
  canceled:   bool,
  tag:        ShortString,
}

impl Default for ConsumerInner {
  fn default() -> Self {
    Self {
      deliveries: VecDeque::new(),
      task:       None,
      canceled:   false,
      tag:        ShortString::default(),
    }
  }
}

impl ConsumerSubscriber for Consumer {
  fn new_delivery(&self, delivery: Delivery) {
    trace!("new_delivery;");
    let mut inner = self.inner.lock();
    inner.deliveries.push_back(delivery);
    if let Some(task) = inner.task.as_ref() {
      task.notify();
    }
  }
  fn drop_prefetched_messages(&self) {
    trace!("drop_prefetched_messages;");
    self.inner.lock().deliveries.clear();
  }
  fn cancel(&self) {
    trace!("cancel;");
    let mut inner = self.inner.lock();
    inner.deliveries.clear();
    inner.canceled = true;
    inner.task.take();
  }
}

impl Stream for Consumer {
  type Item = Delivery;
  type Error = Error;

  fn poll(&mut self) -> Poll<Option<Delivery>, Error> {
    trace!("consumer poll; polling transport");
    let mut inner = self.inner.lock();
    trace!("consumer poll; acquired inner lock, consumer_tag={}", inner.tag);
    if inner.task.is_none() {
      inner.task = Some(task::current());
    }
    if let Some(delivery) = inner.deliveries.pop_front() {
      trace!("delivery; consumer_tag={}, delivery_tag={:?}", inner.tag, delivery.delivery_tag);
      Ok(Async::Ready(Some(delivery)))
    } else if inner.canceled {
      trace!("consumer canceled; consumer_tag={}", inner.tag);
      Ok(Async::Ready(None))
    } else {
      trace!("delivery; status=NotReady, consumer_tag={}", inner.tag);
      Ok(Async::NotReady)
    }
  }
}
