use log::trace;
use parking_lot::{Mutex, MutexGuard};

use std::{
  collections::VecDeque,
  fmt,
  sync::Arc,
};

use crate::{
  BasicProperties,
  message::Delivery,
  types::ShortString,
  wait::NotifyReady,
};

pub trait ConsumerDelegate: Send + Sync {
  fn on_new_delivery(&self, delivery: Delivery);
  fn drop_prefetched_messages(&self) {}
  fn on_canceled(&self) {}
}

#[derive(Clone)]
pub struct Consumer {
  inner: Arc<Mutex<ConsumerInner>>,
}

impl Consumer {
  pub(crate) fn new(consumer_tag: ShortString) -> Consumer {
    Consumer {
      inner: Arc::new(Mutex::new(ConsumerInner::new(consumer_tag))),
    }
  }

  pub fn inner(&self) -> MutexGuard<'_, ConsumerInner> {
    self.inner.lock()
  }

  pub fn set_delegate(&self, delegate: Box<dyn ConsumerDelegate>) {
    let mut inner = self.inner();
    for delivery in inner.deliveries.drain(..) {
      delegate.on_new_delivery(delivery);
    }
    inner.delegate = Some(delegate);
  }

  pub(crate) fn start_new_delivery(&mut self, delivery: Delivery) {
    self.inner().current_message = Some(delivery)
  }

  pub(crate) fn set_delivery_properties(&mut self, properties: BasicProperties) {
    if let Some(delivery) = self.inner().current_message.as_mut() {
      delivery.properties = properties;
    }
  }

  pub(crate) fn receive_delivery_content(&mut self, payload: Vec<u8>) {
    if let Some(delivery) = self.inner().current_message.as_mut() {
      delivery.receive_content(payload);
    }
  }

  pub(crate) fn new_delivery_complete(&mut self) {
    let mut inner = self.inner();
    if let Some(delivery) = inner.current_message.take() {
      inner.new_delivery(delivery);
    }
  }

  pub(crate) fn drop_prefetched_messages(&self) {
    self.inner().drop_prefetched_messages();
  }

  pub(crate) fn cancel(&self) {
    self.inner().cancel();
  }
}

pub struct ConsumerInner {
  current_message: Option<Delivery>,
  deliveries:      VecDeque<Delivery>,
  task:            Option<Box<dyn NotifyReady + Send>>,
  canceled:        bool,
  tag:             ShortString,
  delegate:        Option<Box<dyn ConsumerDelegate>>,
}

impl fmt::Debug for ConsumerInner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "ConsumerInnder({})", self.tag)
  }
}

impl ConsumerInner {
  fn new(consumer_tag: ShortString) -> Self {
    Self {
      current_message: None,
      deliveries:      VecDeque::new(),
      task:            None,
      canceled:        false,
      tag:             consumer_tag,
      delegate:        None,
    }
  }

  pub fn next_delivery(&mut self) -> Option<Delivery> {
    self.deliveries.pop_front()
  }

  pub fn set_task(&mut self, task: Box<dyn NotifyReady + Send>) {
    self.task = Some(task);
  }

  pub fn has_task(&self) -> bool {
    self.task.is_some()
  }

  pub fn canceled(&self) -> bool {
    self.canceled
  }

  pub fn tag(&self) -> &ShortString {
    &self.tag
  }

  fn new_delivery(&mut self, delivery: Delivery) {
    trace!("new_delivery; consumer_tag={}", self.tag);
    if let Some(delegate) = self.delegate.as_ref() {
      delegate.on_new_delivery(delivery);
    } else {
      self.deliveries.push_back(delivery);
    }
    if let Some(task) = self.task.as_ref() {
      task.notify();
    }
  }

  fn drop_prefetched_messages(&mut self) {
    trace!("drop_prefetched_messages; consumer_tag={}", self.tag);
    if let Some(delegate) = self.delegate.as_ref() {
      delegate.drop_prefetched_messages();
    }
    self.deliveries.clear();
  }

  fn cancel(&mut self) {
    trace!("cancel; consumer_tag={}", self.tag);
    if let Some(delegate) = self.delegate.as_ref() {
      delegate.on_canceled();
    }
    self.deliveries.clear();
    self.canceled = true;
    self.task.take();
  }
}

impl fmt::Debug for Consumer {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Consumer({})", self.inner().tag())
  }
}

#[cfg(feature = "futures")]
mod futures {
  use super::*;

  use ::futures::stream::Stream;

  use std::{
    pin::Pin,
    task::{Context, Poll},
  };

  use crate::confirmation::futures::Watcher;

  impl Stream for Consumer {
    type Item = Delivery;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
      trace!("consumer poll; polling transport");
      let mut inner = self.inner();
      trace!("consumer poll; acquired inner lock, consumer_tag={}", inner.tag());
      if !inner.has_task() {
        inner.set_task(Box::new(Watcher(cx.waker().clone())));
      }
      if let Some(delivery) = inner.next_delivery() {
        trace!("delivery; consumer_tag={}, delivery_tag={:?}", inner.tag(), delivery.delivery_tag);
        Poll::Ready(Some(delivery))
      } else if inner.canceled() {
        trace!("consumer canceled; consumer_tag={}", inner.tag());
        Poll::Ready(None)
      } else {
        trace!("delivery; status=NotReady, consumer_tag={}", inner.tag());
        Poll::Pending
      }
    }
  }
}
