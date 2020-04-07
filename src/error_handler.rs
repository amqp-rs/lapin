use crate::Error;
use parking_lot::Mutex;
use std::{fmt, sync::Arc};

type ErrorFn = Box<dyn FnMut(Error) + Send + 'static>;

#[derive(Clone)]
pub(crate) struct ErrorHandler {
    handler: Arc<Mutex<Option<ErrorFn>>>,
}

impl ErrorHandler {
    pub(crate) fn set_handler<E: FnMut(Error) + Send + 'static>(&self, handler: E) {
        *self.handler.lock() = Some(Box::new(handler));
    }

    pub(crate) fn on_error(&self, error: Error) {
        if let Some(handler) = self.handler.lock().as_mut() {
            handler(error)
        }
    }
}

impl Default for ErrorHandler {
    fn default() -> Self {
        Self {
            handler: Arc::new(Mutex::new(None)),
        }
    }
}

impl fmt::Debug for ErrorHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorHandler").finish()
    }
}
