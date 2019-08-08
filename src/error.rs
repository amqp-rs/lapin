use amq_protocol::{
  frame::GenError,
  protocol::AMQPClass,
};
use failure::{Backtrace, Context, Fail};

use std::{fmt, io};

use crate::connection_status::ConnectionState;

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
#[derive(Debug)]
pub enum ErrorKind {
  InvalidMethod(AMQPClass),
  InvalidChannel(u16),
  ConnectionRefused,
  NotConnected,
  UnexpectedReply,
  PreconditionFailed,
  ChannelLimitReached,
  InvalidConnectionState(ConnectionState),
  ParsingError(String),
  SerialisationError(GenError),
  IOError(io::Error),
  /// A hack to prevent developers from exhaustively match on the enum's variants
  ///
  /// The purpose of this variant is to let the `ErrorKind` enumeration grow more variants
  /// without it being a breaking change for users. It is planned for the language to provide
  /// this functionnality out of the box, though it has not been [stabilized] yet.
  ///
  /// [stabilized]: https://github.com/rust-lang/rust/issues/44109
  #[doc(hidden)]
  __Nonexhaustive,
}

impl fmt::Display for ErrorKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    use ErrorKind::*;
    match self {
      InvalidMethod(method) => write!(f, "invalid protocol method: {:?}", method),
      InvalidChannel(channel) => write!(f, "invalid channel: {}", channel),
      ConnectionRefused => write!(f, "connection refused"),
      NotConnected => write!(f, "not connected"),
      UnexpectedReply => write!(f, "unexpected reply"),
      PreconditionFailed => write!(f, "precondition failed"),
      ChannelLimitReached => write!(f, "The maximum number of channels for this connection has been reached"),
      InvalidConnectionState(state) => write!(f, "invalid connection state: {:?}", state),
      ParsingError(e) => write!(f, "Failed to parse: {}", e),
      SerialisationError(e) => write!(f, "Failed to serialise: {:?}", e),
      IOError(e) => write!(f, "IO error: {:?}", e),
      __Nonexhaustive => write!(f, "lapin::error::ErrorKind::__Nonexhaustive: this should not be printed"),
    }
  }
}

impl Fail for ErrorKind {
  fn cause(&self) -> Option<&dyn Fail> {
    match self {
      ErrorKind::SerialisationError(cause) => Some(cause),
      ErrorKind::IOError(cause) => Some(cause),
      _ => None,
    }
  }
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
