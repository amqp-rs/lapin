use futures::{Async, Future, Poll, task};
use lapin::confirmation::{Confirmation, NotifyReady};

use crate::Error;

pub struct ConfirmationFuture<T, I=()>(Confirmation<T, I>);

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
    if !self.0.has_subscriber() {
      self.0.subscribe(Box::new(Watcher(task::current())));
    }
    Ok(if let Some(res) = self.0.try_wait() {
      Async::Ready(res?)
    } else {
      Async::NotReady
    })
  }
}

impl<T, I> From<Confirmation<T, I>> for ConfirmationFuture<T, I> {
  fn from(confirmation: Confirmation<T, I>) -> Self {
    Self(confirmation)
  }
}
