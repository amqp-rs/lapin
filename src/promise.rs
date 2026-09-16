use crate::{Error, Result, listener::Listener};
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
    listener: Listener,
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
        let promise = Self::build(marker, None);
        let resolver = promise.resolver();
        (promise, resolver)
    }

    pub(crate) fn new_with_data(marker: &str, data: Result<T>) -> Self {
        Self::build(marker, Some(data))
    }

    fn build(marker: &str, data: Option<Result<T>>) -> Self {
        let shared = Shared::new(data, marker);
        let listener = shared.listener.clone();
        Self { shared, listener }
    }

    pub(crate) fn try_wait(&self) -> Option<Result<T>> {
        self.shared.take()
    }

    fn resolver(&self) -> PromiseResolver<T> {
        PromiseResolver {
            shared: self.shared.clone(),
        }
    }

    pub(crate) async fn forward_errors_to<O>(self, other: PromiseResolver<O>) -> Result<T> {
        let resolved = self.await;
        if let Err(e) = &resolved {
            other.reject(e.clone())
        }
        resolved
    }
}

impl<T> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            self.listener.arm();
            if let Some(data) = self.shared.take() {
                self.listener.disarm();
                return Poll::Ready(data);
            }
            match self.listener.poll(cx) {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub(crate) struct PromiseResolver<T> {
    shared: Arc<Shared<T>>,
}

impl<T> fmt::Debug for PromiseResolver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PromiseResolver {:?}", self.shared.marker)
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

pub(crate) trait Cancelable: fmt::Debug {
    fn cancel(&self, err: Error);
}

impl<T> Cancelable for PromiseResolver<T> {
    fn cancel(&self, err: Error) {
        self.reject(err)
    }
}

struct Shared<T> {
    data: Mutex<Option<Result<T>>>,
    listener: Listener,
    marker: Option<String>,
}

// Listener wraps event-listener::Event which uses UnsafeCell internally, opting
// out of RefUnwindSafe by default. The only panic vector is Waker::wake() inside
// notify(); if it panics the waiting task is not woken, but the data is already
// written before notify() is called so a subsequent poll will find it.
impl<T> RefUnwindSafe for Shared<T> where Result<T>: RefUnwindSafe {}

impl<T> Shared<T> {
    fn new(data: Option<Result<T>>, marker: &str) -> Arc<Self> {
        Arc::new(Self {
            data: Mutex::new(data),
            listener: Listener::default(),
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
            self.listener.notify();
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
