use crate::{
    channel_status::ChannelState, connection_status::ConnectionState, notifier::Notifier,
    protocol::AMQPError, types::ChannelId,
};
use amq_protocol::{
    frame::{GenError, ParserError, ProtocolVersion},
    protocol::AMQPErrorKind,
};
use std::{error, fmt, io, sync::Arc};

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
    ChannelsLimitReached,
    InvalidProtocolVersion(ProtocolVersion),

    InvalidChannel(ChannelId),
    InvalidChannelState(ChannelState, &'static str),
    InvalidConnectionState(ConnectionState),

    IOError(Arc<io::Error>),
    ParsingError(ParserError),
    ProtocolError(AMQPError),
    SerialisationError(Arc<GenError>),

    MissingHeartbeatError,

    NoConfiguredExecutor,
    NoConfiguredReactor,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    #[deprecated(note = "Please use Channel::wait_for_recovery instead")]
    pub fn notifier(&self) -> Option<Notifier> {
        self.notifier.clone()
    }

    pub(crate) fn with_notifier(mut self, notifier: Option<Notifier>) -> Self {
        self.notifier = notifier;
        self
    }

    pub fn wouldblock(&self) -> bool {
        if let ErrorKind::IOError(e) = self.kind() {
            e.kind() == io::ErrorKind::WouldBlock
        } else {
            false
        }
    }

    pub fn interrupted(&self) -> bool {
        if let ErrorKind::IOError(e) = self.kind() {
            e.kind() == io::ErrorKind::Interrupted
        } else {
            false
        }
    }

    pub fn is_amqp_error(&self) -> bool {
        if let ErrorKind::ProtocolError(_) = self.kind() {
            return true;
        }
        false
    }

    pub fn is_amqp_soft_error(&self) -> bool {
        if let ErrorKind::ProtocolError(e) = self.kind() {
            if let AMQPErrorKind::Soft(_) = e.kind() {
                return true;
            }
        }
        false
    }

    pub fn is_amqp_hard_error(&self) -> bool {
        if let ErrorKind::ProtocolError(e) = self.kind() {
            if let AMQPErrorKind::Hard(_) = e.kind() {
                return true;
            }
        }
        false
    }
}

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
            ErrorKind::ParsingError(e) => write!(f, "failed to parse: {e}"),
            ErrorKind::ProtocolError(e) => write!(f, "protocol error: {e}"),
            ErrorKind::SerialisationError(e) => write!(f, "failed to serialise: {e}"),

            ErrorKind::MissingHeartbeatError => {
                write!(f, "no heartbeat received from server for too long")
            }

            ErrorKind::NoConfiguredExecutor => {
                write!(
                    f,
                    "an executor must be provided if the default-runtime feature is disabled"
                )
            }
            ErrorKind::NoConfiguredReactor => {
                write!(
                    f,
                    "a reactor must be provided if the default-runtime feature is disabled"
                )
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.kind() {
            ErrorKind::IOError(e) => Some(&**e),
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
        use tracing::error;

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

            (IOError(_), IOError(_)) => {
                error!("Unable to compare lapin::ErrorKind::IOError");
                false
            }
            (ParsingError(left_inner), ParsingError(right_inner)) => left_inner == right_inner,
            (ProtocolError(left_inner), ProtocolError(right_inner)) => left_inner == right_inner,
            (SerialisationError(_), SerialisationError(_)) => {
                error!("Unable to compare lapin::ErrorKind::SerialisationError");
                false
            }

            _ => false,
        }
    }
}
