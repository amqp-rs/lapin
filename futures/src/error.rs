use failure::{Backtrace, Context, Fail};
use lapin_async;
use tokio_timer;

use std::fmt;

use crate::transport::CodecError;

/// The type of error that can be returned in this crate.
///
/// Instead of implementing the `Error` trait provided by the standard library,
/// it implemented the `Fail` trait provided by the `failure` crate. Doing so
/// means that this type guaranteed to be both sendable and usable across
/// threads, and that you'll be able to use the downcasting feature of the
/// `failure::Error` type.
#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

/// The different kinds of errors that can be reported.
///
/// Even though we expose the complete enumeration of possible error variants, it is not
/// considered stable to exhaustively match on this enumeration: do it at your own risk.
#[derive(Debug, Fail)]
#[allow(clippy::large_enum_variant)]
pub enum ErrorKind {
    #[fail(display = "Failed to open channel")]
    ChannelOpenFailed,
    #[fail(display = "Couldn't decode incoming frame: {}", _0)]
    Decode(CodecError),
    #[fail(display = "The connection was closed by the remote peer")]
    ConnectionClosed,
    #[fail(display = "Basic get returned empty")]
    EmptyBasicGet,
    #[fail(display = "Couldn't encode outcoming frame: {}", _0)]
    Encode(CodecError),
    #[fail(display = "The timer of the heartbeat encountered an error: {}", _0)]
    HeartbeatTimer(#[fail(cause)] tokio_timer::Error),
    #[fail(display = "Failed to handle incoming frame: {:?}", _0)]
    InvalidFrame(#[fail(cause)] lapin_async::error::Error),
    #[fail(display = "Couldn't parse URI: {}", _0)]
    InvalidUri(String),
    #[fail(display = "{}: {:?}", _0, _1)]
    ProtocolError(String, #[fail(cause)] lapin_async::error::Error),
    /// A hack to prevent developers from exhaustively match on the enum's variants
    ///
    /// The purpose of this variant is to let the `ErrorKind` enumeration grow more variants
    /// without it being a breaking change for users. It is planned for the language to provide
    /// this functionnality out of the box, though it has not been [stabilized] yet.
    ///
    /// [stabilized]: https://github.com/rust-lang/rust/issues/44109
    #[doc(hidden)]
    #[fail(display = "lapin_futures::error::ErrorKind::__Nonexhaustive: this should not be printed")]
    __Nonexhaustive,
}

impl Error {
    /// Return the underlying `ErrorKind`
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error { inner: Context::new(kind) }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}
