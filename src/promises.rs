use crate::pinky_swear::PinkySwear;
use log::trace;
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Debug)]
pub(crate) struct Promises<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

#[derive(Debug)]
struct Inner<T> {
    promises: Vec<PinkySwear<T>>,
}

impl<T> Default for Promises<T> {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }
}

impl<T> Default for Inner<T> {
    fn default() -> Self {
        Self {
            promises: Vec::default(),
        }
    }
}

impl<T: Send + 'static> Promises<T> {
    pub(crate) fn register(&self, promise: PinkySwear<T>) -> Option<T> {
        promise.try_wait().or_else(|| {
            self.inner.lock().promises.push(promise);
            None
        })
    }

    pub(crate) fn try_wait(&self) -> Vec<T> {
        self.inner.lock().try_wait()
    }
}

impl<T: Send + 'static> Inner<T> {
    fn try_wait(&mut self) -> Vec<T> {
        let mut res = Vec::default();
        for promise in std::mem::take(&mut self.promises) {
            if let Some(r) = promise.try_wait() {
                res.push(r);
            } else {
                trace!("Promise wasn't ready yet, storing it back");
                self.promises.push(promise);
            }
        }
        res
    }
}
