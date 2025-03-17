use crate::Error;
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

type ErrorFn = Box<dyn FnMut(Error) + Send + 'static>;
type Inner = Option<ErrorFn>;

#[derive(Clone)]
pub(crate) struct ErrorHandler(Arc<Mutex<Inner>>);

impl ErrorHandler {
    pub(crate) fn set_handler<E: FnMut(Error) + Send + 'static>(&self, handler: E) {
        *self.lock_inner() = Some(Box::new(handler));
    }

    pub(crate) fn on_error(&self, error: Error) {
        if let Some(handler) = self.lock_inner().as_mut() {
            handler(error)
        }
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
}

impl fmt::Debug for ErrorHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ErrorHandler").finish()
    }
}
