use crate::{
    ChannelState, ConnectionState, notifier::Notifier, protocol::AMQPError, types::ChannelId,
};
use amq_protocol::{
    frame::{GenError, ParserError, ProtocolVersion},
    protocol::AMQPErrorKind,
};
use async_rs::{Runtime, traits::*};
use std::{
    error, fmt, io,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::Arc,
};

/// A std Result with a lapin::Error error type
pub type Result<T> = std::result::Result<T, Error>;

/// The error that can be returned in this crate.
#[derive(Clone, Debug)]
pub struct Error {
    kind: ErrorKind,
    notifier: Option<Notifier>,
}

/// The type of error that can be returned in this crate.
///
/// Even though we expose the complete enumeration of possible error variants, it is not
/// considered stable to exhaustively match on this enumeration: do it at your own risk.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The maximum number of channels allowed on this connection has been reached.
    ChannelsLimitReached,
    /// The server only supports an AMQP version that this client does not speak.
    InvalidProtocolVersion(ProtocolVersion),

    /// An operation was attempted on a channel number that does not exist.
    InvalidChannel(ChannelId),
    /// An operation was attempted while the channel was in an incompatible state.
    InvalidChannelState(ChannelState, &'static str),
    /// An operation was attempted while the connection was in an incompatible state.
    InvalidConnectionState(ConnectionState),

    /// An underlying IO error occurred (e.g. connection reset, broken pipe).
    IOError(Arc<io::Error>),
    /// The async runtime was shut down while an IO operation was in progress.
    RuntimeShutdownError(Arc<io::Error>),
    /// The AMQP frame parser encountered malformed data.
    ParsingError(ParserError),
    /// The broker sent an AMQP error (channel or connection level).
    ProtocolError(AMQPError),
    /// An AMQP frame could not be serialised.
    SerialisationError(Arc<GenError>),
    /// The authentication provider returned an error.
    AuthProviderError(String),
    /// A [`crate::PublisherConfirm`] future was polled after it had already resolved.
    FutureCompleted,
    /// No default async runtime is available (no runtime feature flag was enabled).
    NoDefaultRuntime,

    /// The broker did not send a heartbeat within the negotiated timeout.
    MissingHeartbeatError,

    /// An operation timed out.
    // Used in io_loop::with_timeout
    TimeoutError,
}

impl Error {
    pub(crate) fn other<E: Into<Box<dyn error::Error + Send + Sync>>>(error: E) -> Self {
        io::Error::other(error).into()
    }

    pub(crate) fn io<RK: RuntimeKit>(error: io::Error, rt: &Runtime<RK>) -> Self {
        if rt.is_runtime_shutdown_error(&error) {
            ErrorKind::RuntimeShutdownError(Arc::new(error)).into()
        } else {
            error.into()
        }
    }

    /// Return the specific error kind.
    #[must_use]
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub(crate) fn notifier(&self) -> Option<Notifier> {
        self.notifier.clone()
    }

    pub(crate) fn with_notifier(mut self, notifier: Option<Notifier>) -> Self {
        self.notifier = notifier;
        self
    }

    /// Returns `true` if this is an IO error with `WouldBlock` kind.
    #[must_use]
    pub fn wouldblock(&self) -> bool {
        matches!(self.kind(), ErrorKind::IOError(e) if e.kind() == io::ErrorKind::WouldBlock)
    }

    /// Returns `true` if this is an IO error with `Interrupted` kind.
    #[must_use]
    pub fn interrupted(&self) -> bool {
        matches!(self.kind(), ErrorKind::IOError(e) if e.kind() == io::ErrorKind::Interrupted)
    }

    /// Returns `true` if this is an [`ErrorKind::IOError`] or
    /// [`ErrorKind::RuntimeShutdownError`].
    #[must_use]
    pub fn is_io_error(&self) -> bool {
        matches!(self.kind(), ErrorKind::IOError(_)) || self.is_runtime_shutdown_error()
    }

    /// Returns `true` if this is an [`ErrorKind::RuntimeShutdownError`].
    #[must_use]
    pub fn is_runtime_shutdown_error(&self) -> bool {
        matches!(self.kind(), ErrorKind::RuntimeShutdownError(_))
    }

    /// Returns `true` if this is an [`ErrorKind::ProtocolError`].
    #[must_use]
    pub fn is_amqp_error(&self) -> bool {
        matches!(self.kind(), ErrorKind::ProtocolError(_))
    }

    /// Returns `true` if this is a channel-level (soft) AMQP protocol error.
    #[must_use]
    pub fn is_amqp_soft_error(&self) -> bool {
        matches!(self.kind(), ErrorKind::ProtocolError(e) if matches!(e.kind(), AMQPErrorKind::Soft(_)))
    }

    /// Returns `true` if this is a connection-level (hard) AMQP protocol error.
    #[must_use]
    pub fn is_amqp_hard_error(&self) -> bool {
        matches!(self.kind(), ErrorKind::ProtocolError(e) if matches!(e.kind(), AMQPErrorKind::Hard(_)))
    }

    /// Returns `true` if automatic recovery can be attempted for this error.
    ///
    /// Used internally by the auto-recovery logic. Requires
    /// [`ConnectionProperties::enable_auto_recover`] to be set.
    ///
    /// [`ConnectionProperties::enable_auto_recover`]: crate::ConnectionProperties::enable_auto_recover
    #[must_use]
    pub fn can_be_recovered(&self) -> bool {
        match self.kind() {
            ErrorKind::ChannelsLimitReached => false,
            ErrorKind::InvalidProtocolVersion(_) => false,

            ErrorKind::InvalidChannel(_) => true,
            ErrorKind::InvalidChannelState(..) => true,
            ErrorKind::InvalidConnectionState(_) => true,

            ErrorKind::IOError(_) => true,
            ErrorKind::RuntimeShutdownError(_) => false,
            ErrorKind::ParsingError(_) => false,
            ErrorKind::ProtocolError(_) => true,
            ErrorKind::SerialisationError(_) => false,
            ErrorKind::AuthProviderError(_) => false,
            ErrorKind::FutureCompleted => false,
            ErrorKind::NoDefaultRuntime => false,

            ErrorKind::MissingHeartbeatError => true,

            ErrorKind::TimeoutError => true,
        }
    }
}

// io::Error can contain Box<dyn Error + Send + Sync>, which opts out of RefUnwindSafe
// even though the data is behind Arc (immutable shared reference). Error values carry
// no interior mutability of their own; a panic through code holding an Error cannot
// corrupt any invariant.
impl UnwindSafe for Error {}
impl RefUnwindSafe for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            ErrorKind::ChannelsLimitReached => write!(
                f,
                "the maximum number of channels for this connection has been reached"
            ),
            ErrorKind::InvalidProtocolVersion(version) => {
                write!(f, "the server only supports AMQP {version}")
            }

            ErrorKind::InvalidChannel(channel) => write!(f, "invalid channel: {channel}"),
            ErrorKind::InvalidChannelState(state, context) => {
                write!(f, "invalid channel state: {state:?} ({context})")
            }
            ErrorKind::InvalidConnectionState(state) => {
                write!(f, "invalid connection state: {state:?}")
            }

            ErrorKind::IOError(e) => write!(f, "IO error: {e}"),
            ErrorKind::RuntimeShutdownError(e) => write!(f, "runtime shutdown error: {e}"),
            ErrorKind::ParsingError(e) => write!(f, "failed to parse: {e}"),
            ErrorKind::ProtocolError(e) => write!(f, "protocol error: {e}"),
            ErrorKind::SerialisationError(e) => write!(f, "failed to serialise: {e}"),
            ErrorKind::AuthProviderError(e) => write!(f, "failure during authentication: {e}"),
            ErrorKind::FutureCompleted => write!(f, "future polled after completion"),
            ErrorKind::NoDefaultRuntime => write!(f, "no default configured runtime"),

            ErrorKind::MissingHeartbeatError => {
                write!(f, "no heartbeat received from server for too long")
            }
            ErrorKind::TimeoutError => {
                write!(f, "Operation timed out")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.kind() {
            ErrorKind::IOError(e) => Some(&**e),
            ErrorKind::RuntimeShutdownError(e) => Some(&**e),
            ErrorKind::ParsingError(e) => Some(e),
            ErrorKind::ProtocolError(e) => Some(e),
            ErrorKind::SerialisationError(e) => Some(&**e),
            _ => None,
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {
            kind,
            notifier: None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(other: io::Error) -> Self {
        ErrorKind::IOError(Arc::new(other)).into()
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        use ErrorKind::*;

        match (self.kind(), other.kind()) {
            (ChannelsLimitReached, ChannelsLimitReached) => true,
            (InvalidProtocolVersion(left_inner), InvalidProtocolVersion(right_version)) => {
                left_inner == right_version
            }

            (InvalidChannel(left_inner), InvalidChannel(right_inner)) => left_inner == right_inner,
            (
                InvalidChannelState(left_inner, left_context),
                InvalidChannelState(right_inner, right_context),
            ) => left_inner == right_inner && left_context == right_context,
            (InvalidConnectionState(left_inner), InvalidConnectionState(right_inner)) => {
                left_inner == right_inner
            }

            (IOError(_), IOError(_)) => false,
            (RuntimeShutdownError(_), RuntimeShutdownError(_)) => false,
            (ParsingError(left_inner), ParsingError(right_inner)) => left_inner == right_inner,
            (ProtocolError(left_inner), ProtocolError(right_inner)) => left_inner == right_inner,
            (SerialisationError(_), SerialisationError(_)) => false,
            (FutureCompleted, FutureCompleted) => true,
            (NoDefaultRuntime, NoDefaultRuntime) => true,
            (TimeoutError, TimeoutError) => true,
            _ => false,
        }
    }
}
