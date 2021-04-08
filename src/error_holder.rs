use crate::{Error, Result};

use std::{fmt, sync::Arc};

use parking_lot::Mutex;

#[derive(Clone, Default)]
pub(crate) struct ErrorHolder(Arc<Mutex<Option<Error>>>);

impl ErrorHolder {
    pub(crate) fn set(&self, error: Error) {
        *self.0.lock() = Some(error);
    }

    pub(crate) fn check(&self) -> Result<()> {
        self.0
            .lock()
            .clone()
            .map(Err)
            .transpose()
            .map(Option::unwrap_or_default)
    }
}

impl fmt::Debug for ErrorHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorHolder").finish()
    }
}
