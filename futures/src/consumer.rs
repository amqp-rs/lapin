use futures::{Async, Poll, Stream, task};
use lapin_async::consumer::ConsumerSubscriber;
use log::trace;
use parking_lot::Mutex;
use tokio_io::{AsyncRead, AsyncWrite};

use std::collections::VecDeque;
use std::sync::Arc;

use crate::error::Error;
use crate::message::Delivery;
use crate::transport::*;

#[derive(Clone,Debug)]
pub struct ConsumerSub {
  inner: Arc<Mutex<ConsumerInner>>,
}

impl ConsumerSubscriber for ConsumerSub {
  fn new_delivery(&mut self, delivery: Delivery) {
    trace!("new_delivery;");
    let mut inner = self.inner.lock();
    inner.deliveries.push_back(delivery);
    if let Some(task) = inner.task.as_ref() {
      task.notify();
    }
  }
  fn drop_prefetched_messages(&mut self) {
    trace!("drop_prefetched_messages;");
    let mut inner = self.inner.lock();
    inner.deliveries.clear();
  }
}

#[derive(Clone)]
pub struct Consumer<T> {
  transport:    Arc<Mutex<AMQPTransport<T>>>,
  inner:        Arc<Mutex<ConsumerInner>>,
  channel_id:   u16,
  queue:        String,
  consumer_tag: String,
}

#[derive(Debug)]
struct ConsumerInner {
  deliveries: VecDeque<Delivery>,
  task:       Option<task::Task>,
}

impl Default for ConsumerInner {
  fn default() -> Self {
    Self {
      deliveries: VecDeque::new(),
      task:       None,
    }
  }
}

impl<T: AsyncRead+AsyncWrite+Sync+Send+'static> Consumer<T> {
  pub fn new(transport: Arc<Mutex<AMQPTransport<T>>>, channel_id: u16, queue: String, consumer_tag: String) -> Consumer<T> {
    Consumer {
      transport,
      inner: Arc::new(Mutex::new(ConsumerInner::default())),
      channel_id,
      queue,
      consumer_tag,
    }
  }

  pub fn update_consumer_tag(&mut self, consumer_tag: String) {
    self.consumer_tag = consumer_tag;
  }

  pub fn subscriber(&self) -> ConsumerSub {
    ConsumerSub {
      inner: self.inner.clone(),
    }
  }
}

impl<T: AsyncRead+AsyncWrite+Sync+Send+'static> Stream for Consumer<T> {
  type Item = Delivery;
  type Error = Error;

  fn poll(&mut self) -> Poll<Option<Delivery>, Error> {
    trace!("consumer poll; consumer_tag={:?} polling transport", self.consumer_tag);
    let mut transport = self.transport.lock();
    transport.poll()?;
    let mut inner = self.inner.lock();
    trace!("consumer poll; consumer_tag={:?} acquired inner lock", self.consumer_tag);
    if inner.task.is_none() {
      let task = task::current();
      task.notify();
      inner.task = Some(task);
    }
    if let Some(delivery) = inner.deliveries.pop_front() {
      trace!("delivery; consumer_tag={:?} delivery_tag={:?}", self.consumer_tag, delivery.delivery_tag);
      Ok(Async::Ready(Some(delivery)))
    } else {
      trace!("delivery; consumer_tag={:?} status=NotReady", self.consumer_tag);
      Ok(Async::NotReady)
    }
  }
}
