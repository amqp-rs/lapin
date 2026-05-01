use crate::{Error, Result};

use std::{
    fmt,
    sync::{Arc, OnceLock},
};

#[derive(Clone, Default)]
pub(crate) struct ErrorHolder(Arc<OnceLock<Error>>);

impl ErrorHolder {
    pub(crate) fn set(&self, error: Error) -> Result<()> {
        self.0.set(error)
    }

    pub(crate) fn check(&self) -> Result<()> {
        match self.0.get() {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

impl fmt::Debug for ErrorHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorHolder").finish()
    }
}
