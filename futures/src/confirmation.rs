use futures::{Async, Future, Poll, task};
use lapin_async::{
  confirmation::Confirmation,
  wait::NotifyReady,
};

use crate::error::Error;

pub struct ConfirmationFuture<T>(Result<Confirmation<T>, Option<Error>>);

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
    match &mut self.0 {
      Err(err)         => Err(err.take().expect("ConfirmationFuture polled twice but we were in an error state")),
      Ok(confirmation) => match confirmation.try_wait() {
        Some(res) => Ok(Async::Ready(res)),
        None      => {
          confirmation.subscribe(Box::new(Watcher(task::current())));
          Ok(Async::NotReady)
        }
      },
    }
  }
}

impl<T> From<Confirmation<T>> for ConfirmationFuture<T> {
  fn from(confirmation: Confirmation<T>) -> Self {
    Self(confirmation.into_result().map_err(Some))
  }
}
