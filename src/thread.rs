use crate::Result;
use parking_lot::Mutex;
use std::{sync::Arc, thread};

pub(crate) type JoinHandle = thread::JoinHandle<Result<()>>;

#[derive(Clone)]
pub(crate) struct ThreadHandle {
    handle: Arc<Mutex<Option<JoinHandle>>>,
}

impl Default for ThreadHandle {
    fn default() -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl ThreadHandle {
    pub(crate) fn register(&self, handle: JoinHandle) {
        *self.handle.lock() = Some(handle);
    }

    pub(crate) fn wait(&self, context: &'static str) -> Result<()> {
        if let Some(handle) = self.handle.lock().take() {
            handle.join().expect(context)?
        }
        Ok(())
    }
}
