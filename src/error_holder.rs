use crate::{Error, Result};

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

type Inner = Option<Error>;

#[derive(Clone, Default)]
pub(crate) struct ErrorHolder(Arc<Mutex<Inner>>);

impl ErrorHolder {
    pub(crate) fn set(&self, error: Error) {
        *self.lock_inner() = Some(error);
    }

    pub(crate) fn check(&self) -> Result<()> {
        self.lock_inner()
            .clone()
            .map(Err)
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for ErrorHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorHolder").finish()
    }
}
