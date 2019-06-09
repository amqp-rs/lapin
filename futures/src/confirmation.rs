use futures::{Async, Future, Poll, task};
use lapin_async::confirmation::{Confirmation, NotifyReady};

use crate::Error;

pub struct ConfirmationFuture<T> {
  inner:     Result<Confirmation<T>, Option<Error>>,
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
    match &mut self.inner {
      Err(err)         => Err(err.take().expect("ConfirmationFuture polled twice but we were in an error state")),
      Ok(confirmation) => {
        if !self.subsribed {
          confirmation.subscribe(Box::new(Watcher(task::current())));
          self.subsribed = true;
        }
        Ok(confirmation.try_wait().map(Async::Ready).unwrap_or(Async::NotReady))
      },
    }
  }
}

impl<T> From<Confirmation<T>> for ConfirmationFuture<T> {
  fn from(confirmation: Confirmation<T>) -> Self {
    Self { inner: confirmation.into_result().map_err(Some), subsribed: false }
  }
}
