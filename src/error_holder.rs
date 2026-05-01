use crate::{Error, Result};

use std::{
    fmt,
    panic::{RefUnwindSafe, UnwindSafe},
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

// io::Error can contain Box<dyn Error + Send + Sync>, which is not RefUnwindSafe,
// so Error (and therefore OnceLock<Error>) opts out automatically. ErrorHolder is
// a write-once wrapper: set() initialises the OnceLock at most once, and check()
// only reads it. A panic that unwinds through code holding an ErrorHolder (owned
// or by reference) cannot leave the OnceLock in a partially-written or otherwise
// corrupt state.
impl UnwindSafe for ErrorHolder {}
impl RefUnwindSafe for ErrorHolder {}

impl fmt::Debug for ErrorHolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorHolder").finish()
    }
}
