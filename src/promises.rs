use crate::{Promise, Result};
use log::trace;
use parking_lot::Mutex;
use std::{fmt, sync::Arc};

// FIXME: drop Inner
#[derive(Clone)]
pub(crate) struct Promises<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

struct Inner<T> {
    promises: Vec<Promise<T>>,
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
    pub(crate) fn register(&self, promise: Promise<T>) -> Option<Result<T>> {
        promise.try_wait().or_else(|| {
            self.inner.lock().promises.push(promise);
            None
        })
    }

    pub(crate) fn try_wait(&self) -> Option<Vec<Result<T>>> {
        self.inner.lock().try_wait()
    }
}

impl<T> fmt::Debug for Promises<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Promises").finish()
    }
}

impl<T: Send + 'static> Inner<T> {
    fn try_wait(&mut self) -> Option<Vec<Result<T>>> {
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
