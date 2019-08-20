use parking_lot::Mutex;

use std::{fmt, sync::Arc};

#[derive(Clone)]
pub(crate) struct ErrorHandler {
    handler: Arc<Mutex<Option<Box<dyn Fn() + Send + 'static>>>>,
}

impl ErrorHandler {
    pub(crate) fn set_handler<E: Fn() + Send + 'static>(&self, handler: Box<E>) {
        *self.handler.lock() = Some(handler);
    }

    pub(crate) fn on_error(&self) {
        if let Some(handler) = self.handler.lock().as_ref() {
            handler()
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
        write!(f, "ErrorHandler")
    }
}
