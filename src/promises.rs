use crate::{Promise, Result};
use log::trace;
use parking_lot::Mutex;
use std::{fmt, sync::Arc};

#[derive(Clone)]
pub(crate) struct Promises<T>(Arc<Mutex<Vec<Promise<T>>>>);

impl<T> Default for Promises<T> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Vec::default())))
    }
}

impl<T: Send + 'static> Promises<T> {
    pub(crate) fn register(&self, promise: Promise<T>) -> Option<Result<T>> {
        promise.try_wait().or_else(|| {
            self.0.lock().push(promise);
            None
        })
    }

    pub(crate) fn try_wait(&self) -> Option<Vec<Result<T>>> {
        let mut promises = self.0.lock();
        let before = promises.len();
        if before != 0 {
            let mut res = Vec::default();
            for promise in std::mem::take(&mut *promises) {
                if let Some(r) = promise.try_wait() {
                    res.push(r);
                } else {
                    trace!("Promise wasn't ready yet, storing it back");
                    promises.push(promise);
                }
            }
            trace!("{} promises left (was {})", promises.len(), before);
            Some(res)
        } else {
            None
        }
    }
}

impl<T> fmt::Debug for Promises<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Promises").finish()
    }
}
