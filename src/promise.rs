use crate::{Error, Result};
use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) struct Promise<T>(pinky_swear::PinkySwear<Result<T>>);

impl<T> fmt::Debug for Promise<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Promise")
    }
}

impl<T: Send + 'static> Promise<T> {
    pub(crate) fn new() -> (Self, PromiseResolver<T>) {
        let (promise, resolver) = pinky_swear::PinkySwear::new();
        (Self(promise), PromiseResolver(resolver))
    }

    pub(crate) fn new_with_data(data: Result<T>) -> Self {
        let (promise, resolver) = Self::new();
        resolver.complete(data);
        promise
    }

    pub(crate) fn set_marker(&self, marker: String) {
        self.0.set_marker(marker)
    }

    pub(crate) fn try_wait(&self) -> Option<Result<T>> {
        self.0.try_wait()
    }
}

impl<T: Send + 'static> Future for Promise<T> {
    type Output = Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

pub trait Cancelable {
    fn cancel(&self, err: Error);
}

pub(crate) struct PromiseResolver<T>(pinky_swear::Pinky<Result<T>>);

impl<T> fmt::Debug for PromiseResolver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Promise")
    }
}

impl<T> Clone for PromiseResolver<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
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
        self.0.swear(res)
    }
}

impl<T> Cancelable for PromiseResolver<T> {
    fn cancel(&self, err: Error) {
        self.reject(err)
    }
}

pub(crate) struct PromisesBroadcaster<T>(pinky_swear::PinkyErrorBroadcaster<T, Error>);

impl<T: Send + 'static> PromisesBroadcaster<T> {
    pub(crate) fn new() -> (Promise<T>, Self) {
        let (promise, broadcaster) = pinky_swear::PinkyErrorBroadcaster::new();
        (Promise(promise), Self(broadcaster))
    }

    pub(crate) fn resolve(&self, data: T) {
        self.complete(Ok(data))
    }

    pub(crate) fn reject(&self, error: Error) {
        self.complete(Err(error))
    }

    fn complete(&self, res: Result<T>) {
        self.0.swear(res)
    }

    pub(crate) fn subscribe(&self) -> Promise<()> {
        Promise(self.0.subscribe())
    }

    pub(crate) fn unsubscribe(&self, promise: Promise<()>) {
        self.0.unsubscribe(promise.0)
    }
}
