pub use crate::wait::NotifyReady;
use crate::{wait::Wait, Error, Result};
use std::fmt;

pub struct Confirmation<T, I = ()> {
    kind: ConfirmationKind<T, I>,
}

impl<T, I> Confirmation<T, I> {
    pub(crate) fn new(wait: Wait<T>) -> Self {
        Self {
            kind: ConfirmationKind::Wait(wait),
        }
    }

    pub(crate) fn new_error(error: Error) -> Self {
        let (wait, wait_handle) = Wait::new();
        wait_handle.error(error);
        Self::new(wait)
    }

    // FIXME: remove
    pub(crate) fn into_error(self) -> Result<()> {
        self.try_wait().transpose().map(|_| ())
    }

    pub fn subscribe(&self, task: Box<dyn NotifyReady + Send>) {
        match &self.kind {
            ConfirmationKind::Wait(wait) => wait.subscribe(task),
            ConfirmationKind::Map(wait, _) => wait.subscribe(task),
        }
    }

    pub fn has_subscriber(&self) -> bool {
        match &self.kind {
            ConfirmationKind::Wait(wait) => wait.has_subscriber(),
            ConfirmationKind::Map(wait, _) => wait.has_subscriber(),
        }
    }

    pub fn try_wait(&self) -> Option<Result<T>> {
        match &self.kind {
            ConfirmationKind::Wait(wait) => wait.try_wait(),
            ConfirmationKind::Map(wait, f) => wait.try_wait().map(|res| res.map(f)),
        }
    }

    pub fn wait(self) -> Result<T> {
        match self.kind {
            ConfirmationKind::Wait(wait) => wait.wait(),
            ConfirmationKind::Map(wait, f) => wait.wait().map(f),
        }
    }
}

impl<T> Confirmation<T> {
    pub(crate) fn map<M>(self, f: Box<dyn Fn(T) -> M + Send + 'static>) -> Confirmation<M, T> {
        Confirmation {
            kind: ConfirmationKind::Map(Box::new(self), f),
        }
    }
}

enum ConfirmationKind<T, I> {
    Wait(Wait<T>),
    Map(Box<Confirmation<I>>, Box<dyn Fn(I) -> T + Send + 'static>),
}

impl<T> From<Result<Wait<T>>> for Confirmation<T> {
    fn from(res: Result<Wait<T>>) -> Self {
        match res {
            Ok(wait) => Confirmation::new(wait),
            Err(err) => Confirmation::new_error(err),
        }
    }
}

impl<T> fmt::Debug for Confirmation<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Confirmation")
    }
}

#[cfg(feature = "futures")]
pub(crate) mod futures {
    use super::*;

    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };

    impl<T, I> Future for Confirmation<T, I> {
        type Output = Result<T>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if !self.has_subscriber() {
                self.subscribe(Box::new(Watcher(cx.waker().clone())));
            }
            self.try_wait().map(Poll::Ready).unwrap_or(Poll::Pending)
        }
    }

    pub(crate) struct Watcher(pub(crate) Waker);

    impl NotifyReady for Watcher {
        fn notify(&self) {
            self.0.wake_by_ref();
        }
    }
}
