use crate::Result;
use mio::Waker as MioWaker;
use parking_lot::RwLock;
use std::{fmt, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct Waker {
    inner: Arc<RwLock<Option<Arc<MioWaker>>>>,
}

impl Waker {
    pub(crate) fn wake(&self) -> Result<()> {
        if let Some(waker) = self.inner.read().as_ref() {
            waker.wake()?;
        }
        Ok(())
    }

    pub(crate) fn set_waker(&self, waker: Arc<MioWaker>) -> Result<()> {
        *self.inner.write() = Some(waker);
        self.wake()
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Waker")
    }
}
