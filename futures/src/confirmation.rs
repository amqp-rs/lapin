use futures::{task, Async, Future, Poll};
use lapin::{
    pinky_swear::{NotifyReady, PinkySwear},
    Result,
};

use crate::Error;

#[deprecated(note = "use lapin instead")]
pub struct ConfirmationFuture<T, I = ()>(PinkySwear<Result<T>, I>);

pub(crate) struct Watcher(task::Task);

impl Default for Watcher {
    fn default() -> Self {
        Self(task::current())
    }
}

impl NotifyReady for Watcher {
    fn notify(&self) {
        self.0.notify();
    }
}

impl<T: Send + 'static, I: 'static> Future for ConfirmationFuture<T, I> {
    type Item = T;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.drop_subscribers();
        self.0.subscribe(Box::new(Watcher::default()));
        Ok(if let Some(res) = self.0.try_wait() {
            Async::Ready(res?)
        } else {
            Async::NotReady
        })
    }
}

impl<T, I> From<PinkySwear<Result<T>, I>> for ConfirmationFuture<T, I> {
    fn from(promise: PinkySwear<Result<T>, I>) -> Self {
        Self(promise)
    }
}
