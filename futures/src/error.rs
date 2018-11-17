use std::fmt;
use failure::{Backtrace, Context, Fail};

/// The type of error that can be returned in this crate.
#[derive(Debug)]
pub struct Error {
    inner: Context<String>,
}

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Error {
            inner: Context::new(message.into())
        }
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}
