use crate::{Error, Result};
use flume::{Receiver, Sender};
use parking_lot::RwLock;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tracing::{trace, warn};

#[must_use = "Promise should be used or you can miss errors"]
pub(crate) struct Promise<T> {
    recv: Receiver<Result<T>>,
    recv_fut: Pin<Box<dyn Future<Output = Result<T>> + Send>>,
    resolver: PromiseResolver<T>,
}

impl<T> fmt::Debug for Promise<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Promise")
    }
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        trace!(
            promise = %self.resolver.marker(),
            "Dropping promise.",
        );
    }
}

impl<T: Send + 'static> Promise<T> {
    pub(crate) fn new() -> (Self, PromiseResolver<T>) {
        let (send, recv) = flume::unbounded();
        let resolver = PromiseResolver {
            send,
            marker: Default::default(),
        };
        let recv_fut = recv.clone().into_recv_async();
        let recv_fut = Box::pin(async move {
            // Since we always hold a ref to sender, we can safely unwrap here
            recv_fut.await.unwrap()
        });
        let promise = Self {
            recv,
            recv_fut,
            resolver,
        };
        let resolver = promise.resolver.clone();
        (promise, resolver)
    }

    pub(crate) fn new_with_data(data: Result<T>) -> Self {
        let (promise, resolver) = Self::new();
        resolver.complete(data);
        promise
    }

    pub(crate) fn set_marker(&self, marker: String) {
        self.resolver.set_marker(marker)
    }

    pub(crate) fn try_wait(&self) -> Option<Result<T>> {
        self.recv.try_recv().ok()
    }
}

impl<T: Send + 'static> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.recv_fut).poll(cx)
    }
}

pub trait Cancelable {
    fn cancel(&self, err: Error);
}

pub(crate) struct PromiseResolver<T> {
    send: Sender<Result<T>>,
    marker: Arc<RwLock<Option<String>>>,
}

impl<T> fmt::Debug for PromiseResolver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PromiseResolver")
    }
}

impl<T> Clone for PromiseResolver<T> {
    fn clone(&self) -> Self {
        Self {
            send: self.send.clone(),
            marker: self.marker.clone(),
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
            promise = %self.marker(),
            "Resolving promise.",
        );
        if let Err(err) = self.send.send(res) {
            warn!(
                promise = %self.marker(),
                error = %err,
                "Failed resolving promise, promise has vanished.",
            );
        }
    }

    fn set_marker(&self, marker: String) {
        *self.marker.write() = Some(marker);
    }

    fn marker(&self) -> String {
        self.marker
            .read()
            .as_ref()
            .map_or(String::default(), |marker| format!("[{}] ", marker))
    }
}

impl<T> Cancelable for PromiseResolver<T> {
    fn cancel(&self, err: Error) {
        self.reject(err)
    }
}
