use failure::{Backtrace, Context, Fail};
use lapin_async;
use std::fmt;
use std::io;
use tokio_timer;

use transport::CodecError;

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
/// This enumeration is deliberately not exported to the end-users of this
/// crate because it is not yet possible to prevent developers of matching
/// exhaustively against its variants.
#[derive(Debug, Fail)]
pub(crate) enum ErrorKind {
    #[fail(display = "The maximum number of channels for this connection has been reached")]
    ChannelLimitReached,
    #[fail(display = "Failed to open channel")]
    ChannelOpenFailed,
    #[fail(display = "Couldn't decode incoming frame: {}", _0)]
    Decode(CodecError),
    #[fail(display = "The connection was closed by the remote peer")]
    ConnectionClosed,
    #[fail(display = "Failed to connect: {}", _0)]
    ConnectionFailed(#[fail(cause)] io::Error),
    #[fail(display = "Basic get returned empty")]
    EmptyBasicGet,
    #[fail(display = "Couldn't encode outcoming frame: {}", _0)]
    Encode(CodecError),
    #[fail(display = "The timer of the heartbeat encountered an error: {}", _0)]
    HeartbeatTimer(#[fail(cause)] tokio_timer::Error),
    #[fail(display = "Failed to handle incoming frame: {:?}", _0)]
    // FIXME: mark lapin_async's Error as cause once it implements Fail
    InvalidFrame(lapin_async::error::Error),
    #[fail(display = "Couldn't parse URI: {}", _0)]
    InvalidUri(String),
    #[fail(display = "Transport mutex is poisoned")]
    PoisonedMutex,
    #[fail(display = "{}: {:?}", _0, _1)]
    // FIXME: mark lapin_async's Error as cause once it implements Fail
    ProtocolError(String, lapin_async::error::Error)
}

impl Error {
    /// Returns true if the error is caused by the limit of channels being reached.
    pub fn is_channel_limit_reached(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::ChannelLimitReached => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a channel that couldn't be opened.
    pub fn is_channel_open_failed(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::ChannelOpenFailed => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a connection closed unexpectedly.
    pub fn is_connection_closed(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::ConnectionClosed => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a connection that couldn't be established.
    pub fn is_connection_failed(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::ConnectionFailed(_) => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a frame that couldn't be decoded.
    pub fn is_decode(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::Decode(_) => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by the `basic.get` response being empty.
    pub fn is_empty_basic_get(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::EmptyBasicGet => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a frame that couldn't be encoded.
    pub fn is_encode(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::Encode(_) => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by the timer used in the heartbeat task.
    pub fn is_heartbeat_timer(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::HeartbeatTimer(_) => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a malformed AMQP URI.
    pub fn is_invalid_uri(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::InvalidUri(_) => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a mutex that got poisoned.
    pub fn is_poisoned_mutex(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::PoisonedMutex => true,
            _ => false,
        }
    }

    /// Returns true if the error is caused by a protocol error.
    pub fn is_protocol_error(&self) -> bool {
        match *self.inner.get_context() {
            ErrorKind::ProtocolError(_, _) => true,
            _ => false,
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

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error { inner: Context::new(kind) }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner: inner }
    }
}
