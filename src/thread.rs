use crate::Result;
use parking_lot::Mutex;
use std::{sync::Arc, thread};

type JoinHandle = thread::JoinHandle<Result<()>>;

#[derive(Clone)]
pub(crate) struct ThreadHandle(Arc<Mutex<Option<JoinHandle>>>);

impl Default for ThreadHandle {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
}

impl ThreadHandle {
    pub(crate) fn new(handle: JoinHandle) -> Self {
        Self(Arc::new(Mutex::new(Some(handle))))
    }

    pub(crate) fn register(&self, handle: JoinHandle) {
        *self.0.lock() = Some(handle);
    }

    pub(crate) fn wait(&self, context: &'static str) -> Result<()> {
        if let Some(handle) = self.0.lock().take() {
            handle.join().expect(context)?
        }
        Ok(())
    }
}
