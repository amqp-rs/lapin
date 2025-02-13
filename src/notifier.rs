use crate::wakers::Wakers;

use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

#[derive(Default, Clone)]
pub struct Notifier {
    done: Arc<AtomicBool>,
    wakers: Arc<Wakers>,
}

impl Notifier {
    pub(crate) fn notify_all(&self) {
        self.done.store(true, Ordering::Release);
        self.wakers.wake();
    }

    fn ready(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }
}

impl Future for Notifier {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.ready() {
            Poll::Ready(())
        } else {
            self.wakers.register(cx.waker());
            Poll::Pending
        }
    }
}

impl fmt::Debug for Notifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Notifier").finish()
    }
}
