use crate::{Error, Result};

use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

type Inner = Option<Error>;

#[derive(Clone, Default)]
pub(crate) struct ErrorHolder(Arc<RwLock<Inner>>);

impl ErrorHolder {
    pub(crate) fn set(&self, error: Error) {
        *self.write() = Some(error);
    }

    pub(crate) fn check(&self) -> Result<()> {
        self.read()
            .clone()
            .map(Err)
            .transpose()
            .map(Option::unwrap_or_default)
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.0.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.0.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for ErrorHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorHolder").finish()
    }
}
