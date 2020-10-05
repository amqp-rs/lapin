use parking_lot::Mutex;
use std::{sync::Arc, task::Waker};

#[derive(Default)]
pub(crate) struct Wakers(Arc<Mutex<Vec<Waker>>>);

impl Wakers {
    pub(crate) fn register(&self, waker: Waker) {
        let mut inner = self.0.lock();
        for w in inner.iter() {
            if w.will_wake(&waker) {
                return;
            }
        }
        inner.push(waker);
    }

    pub(crate) fn wake(&self) {
        for w in self.0.lock().drain(..) {
            w.wake();
        }
    }
}
