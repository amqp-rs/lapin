use crate::{Error, Result};
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll, Waker},
};
use tracing::{Level, level_enabled, trace};

#[must_use = "Promise should be used or you can miss errors"]
pub(crate) struct Promise<T> {
    shared: Arc<Mutex<Shared<T>>>,
}

impl<T> fmt::Debug for Promise<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Promise")
    }
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        trace!(
            promise = %self.shared().marker(),
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
        self.shared().take()
    }

    fn resolver(&self) -> PromiseResolver<T> {
        PromiseResolver {
            shared: self.shared.clone(),
        }
    }

    fn shared(&self) -> MutexGuard<'_, Shared<T>> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl<T> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared = self.shared();
        match shared.take() {
            Some(data) => Poll::Ready(data),
            None => {
                shared.register(cx.waker());
                Poll::Pending
            }
        }
    }
}

pub(crate) struct PromiseResolver<T> {
    shared: Arc<Mutex<Shared<T>>>,
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
        let mut shared = self.shared();
        trace!(
            promise = %shared.marker(),
            "Resolving promise.",
        );
        shared.set(res)
    }

    fn shared(&self) -> MutexGuard<'_, Shared<T>> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
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
    data: Option<Result<T>>,
    marker: Option<String>,
    wakers: Vec<Waker>,
}

impl<T> Shared<T> {
    fn new(data: Option<Result<T>>, marker: &str) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            data,
            marker: if level_enabled!(Level::TRACE) {
                Some(marker.into())
            } else {
                None
            },
            wakers: Vec::new(),
        }))
    }

    fn set(&mut self, data: Result<T>) {
        if self.data.is_none() {
            self.data = Some(data);
            for w in self.wakers.drain(..) {
                w.wake();
            }
        }
    }

    fn take(&mut self) -> Option<Result<T>> {
        self.data.take()
    }

    fn register(&mut self, waker: &Waker) {
        for w in self.wakers.iter() {
            if w.will_wake(waker) {
                return;
            }
        }
        self.wakers.push(waker.clone());
    }

    fn marker(&self) -> String {
        self.marker
            .as_ref()
            .map_or(String::default(), |marker| format!("[{marker}] "))
    }
}
