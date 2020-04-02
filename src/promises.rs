use crate::pinky_swear::PinkySwear;
use log::trace;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct Promises<T, S = ()> {
    inner: Arc<Mutex<Inner<T, S>>>,
}

#[derive(Debug)]
struct Inner<T, S = ()> {
    promises: Vec<PinkySwear<T, S>>,
}

impl<T, S> Default for Promises<T, S> {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }
}

impl<T, S> Default for Inner<T, S> {
    fn default() -> Self {
        Self {
            promises: Vec::default(),
        }
    }
}

impl<T: Send + 'static, S: Send + 'static> Promises<T, S> {
    pub(crate) fn register(&self, promise: PinkySwear<T, S>) -> Option<T> {
        promise.try_wait().or_else(|| {
            self.inner.lock().promises.push(promise);
            None
        })
    }

    pub(crate) fn try_wait(&self) -> Option<Vec<T>> {
        self.inner.lock().try_wait()
    }
}

impl<T: Send + 'static, S: Send + 'static> Inner<T, S> {
    fn try_wait(&mut self) -> Option<Vec<T>> {
        let before = self.promises.len();
        if before != 0 {
            let mut res = Vec::default();
            for promise in std::mem::take(&mut self.promises) {
                if let Some(r) = promise.try_wait() {
                    res.push(r);
                } else {
                    trace!("Promise wasn't ready yet, storing it back");
                    self.promises.push(promise);
                }
            }
            trace!("{} promises left (was {})", self.promises.len(), before);
            Some(res)
        } else {
            None
        }
    }
}
