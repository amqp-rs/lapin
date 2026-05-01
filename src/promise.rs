use crate::{Error, Result};
use atomic_waker::AtomicWaker;
use std::{
    fmt,
    future::Future,
    panic::RefUnwindSafe,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll},
};
use tracing::{Level, level_enabled, trace};

#[must_use = "Promise should be used or you can miss errors"]
pub(crate) struct Promise<T> {
    shared: Arc<Shared<T>>,
}

impl<T> fmt::Debug for Promise<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Promise")
    }
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        trace!(
            promise = %self.shared.marker(),
            "Dropping promise.",
        );
    }
}

impl<T> Promise<T> {
    pub(crate) fn new(marker: &str) -> (Self, PromiseResolver<T>) {
        let promise = Self {
            shared: Shared::new(None, marker),
        };
        let resolver = promise.resolver();
        (promise, resolver)
    }

    pub(crate) fn new_with_data(marker: &str, data: Result<T>) -> Self {
        Self {
            shared: Shared::new(Some(data), marker),
        }
    }

    pub(crate) fn try_wait(&self) -> Option<Result<T>> {
        self.shared.take()
    }

    fn resolver(&self) -> PromiseResolver<T> {
        PromiseResolver {
            shared: self.shared.clone(),
        }
    }
}

impl<T> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Fast path: already resolved.
        if let Some(data) = self.shared.take() {
            return Poll::Ready(data);
        }
        // Register the waker before the second check so we don't miss a
        // wakeup that arrives between the two take() calls.
        self.shared.waker.register(cx.waker());
        match self.shared.take() {
            Some(data) => Poll::Ready(data),
            None => Poll::Pending,
        }
    }
}

pub(crate) struct PromiseResolver<T> {
    shared: Arc<Shared<T>>,
}

impl<T> fmt::Debug for PromiseResolver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PromiseResolver")
    }
}

impl<T> Clone for PromiseResolver<T> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> PromiseResolver<T> {
    pub(crate) fn resolve(&self, data: T) {
        self.complete(Ok(data))
    }

    pub(crate) fn reject(&self, error: Error) {
        self.complete(Err(error))
    }

    pub(crate) fn complete(&self, res: Result<T>) {
        trace!(
            promise = %self.shared.marker(),
            "Resolving promise.",
        );
        self.shared.set(res);
    }
}

pub trait Cancelable {
    fn cancel(&self, err: Error);
}

impl<T> Cancelable for PromiseResolver<T> {
    fn cancel(&self, err: Error) {
        self.reject(err)
    }
}

struct Shared<T> {
    data: Mutex<Option<Result<T>>>,
    waker: AtomicWaker,
    marker: Option<String>,
}

// AtomicWaker uses UnsafeCell internally, which opts out of RefUnwindSafe by
// default. The only panic vector inside AtomicWaker is Waker::clone(); if that
// panics, the waker's atomic state machine gets stuck. This is not a broke
// invariant that could cause further unsoundness in code that catches the unwind.
impl<T> RefUnwindSafe for Shared<T> where Result<T>: RefUnwindSafe {}

impl<T> Shared<T> {
    fn new(data: Option<Result<T>>, marker: &str) -> Arc<Self> {
        Arc::new(Self {
            data: Mutex::new(data),
            waker: AtomicWaker::new(),
            marker: if level_enabled!(Level::TRACE) {
                Some(marker.into())
            } else {
                None
            },
        })
    }

    fn set(&self, data: Result<T>) {
        let mut lock = self.lock_data();
        if lock.is_none() {
            *lock = Some(data);
            // Release the lock before waking to avoid the woken task
            // immediately blocking on it.
            drop(lock);
            self.waker.wake();
        }
    }

    fn take(&self) -> Option<Result<T>> {
        self.lock_data().take()
    }

    fn lock_data(&self) -> MutexGuard<'_, Option<Result<T>>> {
        self.data.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn marker(&self) -> String {
        self.marker
            .as_ref()
            .map_or(String::default(), |marker| format!("[{marker}] "))
    }
}
