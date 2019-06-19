use futures::{Async, Future, Poll, task};
use lapin_async::confirmation::{Confirmation, NotifyReady};

use crate::Error;

pub struct ConfirmationFuture<T, I=()> {
  inner:     Confirmation<T, I>,
  subsribed: bool,
}

struct Watcher(task::Task);

impl NotifyReady for Watcher {
  fn notify(&self) {
    self.0.notify();
  }
}

impl<T, I> Future for ConfirmationFuture<T, I> {
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

impl<T, I> From<Confirmation<T, I>> for ConfirmationFuture<T, I> {
  fn from(confirmation: Confirmation<T, I>) -> Self {
    Self { inner: confirmation, subsribed: false }
  }
}
