use amq_protocol::protocol::AMQPClass;
use failure::{Backtrace, Context, Fail};

use std::fmt;

use crate::api::ChannelState;

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
#[derive(Debug, PartialEq, Fail)]
pub enum ErrorKind {
  #[fail(display = "output buffer is too small")]
  SendBufferTooSmall,
  #[fail(display = "input buffer is too small")]
  ReceiveBufferTooSmall,
  #[fail(display = "invalid channel state, expected {:?}, got {:?}", expected, actual)]
  InvalidState { expected: ChannelState, actual: ChannelState },
  #[fail(display = "invalid protocol method: {:?}", _0)]
  InvalidMethod(AMQPClass),
  #[fail(display = "invalid channel: {}", _0)]
  InvalidChannel(u16),
  #[fail(display = "not connected")]
  NotConnected,
  #[fail(display = "unexpected answer")]
  UnexpectedAnswer,
  #[fail(display = "precondition failed")]
  PreconditionFailed,
  /// A hack to prevent developers from exhaustively match on the enum's variants
  ///
  /// The purpose of this variant is to let the `ErrorKind` enumeration grow more variants
  /// without it being a breaking change for users. It is planned for the language to provide
  /// this functionnality out of the box, though it has not been [stabilized] yet.
  ///
  /// [stabilized]: https://github.com/rust-lang/rust/issues/44109
  #[doc(hidden)]
  #[fail(display = "lapin_async::error::ErrorKind::__Nonexhaustive: this should not be printed")]
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
