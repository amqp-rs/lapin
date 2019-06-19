use futures::{Async, Future, Poll, task};
use lapin_async::confirmation::{Confirmation, NotifyReady};

use crate::Error;

pub struct ConfirmationFuture<T> {
  inner:     Confirmation<T>,
  subsribed: bool,
}

struct Watcher(task::Task);

impl NotifyReady for Watcher {
  fn notify(&self) {
    self.0.notify();
  }
}

impl<T> Future for ConfirmationFuture<T> {
  type Item = T;
  type Error = Error;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    if !self.subsribed {
      self.inner.subscribe(Box::new(Watcher(task::current())));
      self.subsribed = true;
    }
    Ok(if let Some(res) = self.inner.try_wait() {
      Async::Ready(res?)
    } else {
      Async::NotReady
    })
  }
}

impl<T> From<Confirmation<T>> for ConfirmationFuture<T> {
  fn from(confirmation: Confirmation<T>) -> Self {
    Self { inner: confirmation, subsribed: false }
  }
}
