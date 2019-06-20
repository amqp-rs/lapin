use futures::{Async, Poll, Stream};
use lapin::streaming_consumer::StreamingConsumer;
use log::trace;

use crate::{
  Error,
  confirmation::Watcher,
  message::Delivery,
};

#[derive(Clone, Debug, Default)]
pub struct Consumer(StreamingConsumer);

impl Consumer {
  pub(crate) fn subscriber(&self) -> StreamingConsumer {
    self.0.clone()
  }
}

impl Stream for Consumer {
  type Item = Delivery;
  type Error = Error;

  fn poll(&mut self) -> Poll<Option<Delivery>, Error> {
    trace!("consumer poll; polling transport");
    let mut inner = self.0.inner();
    trace!("consumer poll; acquired inner lock, consumer_tag={}", inner.tag());
    if !inner.has_task() {
      inner.set_task(Box::new(Watcher::default()));
    }
    if let Some(delivery) = inner.next_delivery() {
      trace!("delivery; consumer_tag={}, delivery_tag={:?}", inner.tag(), delivery.delivery_tag);
      Ok(Async::Ready(Some(delivery)))
    } else if inner.canceled() {
      trace!("consumer canceled; consumer_tag={}", inner.tag());
      Ok(Async::Ready(None))
    } else {
      trace!("delivery; status=NotReady, consumer_tag={}", inner.tag());
      Ok(Async::NotReady)
    }
  }
}
