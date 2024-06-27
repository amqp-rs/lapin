use crate::{
    channel_status::ChannelState, connection_status::ConnectionState, protocol::AMQPError,
    types::ChannelId,
};
use amq_protocol::frame::{GenError, ParserError, ProtocolVersion};
use std::{error, fmt, io, sync::Arc};

/// A std Result with a lapin::Error error type
pub type Result<T> = std::result::Result<T, Error>;

/// The type of error that can be returned in this crate.
///
/// Even though we expose the complete enumeration of possible error variants, it is not
/// considered stable to exhaustively match on this enumeration: do it at your own risk.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Error {
    ChannelsLimitReached,
    InvalidProtocolVersion(ProtocolVersion),

    InvalidChannel(ChannelId),
    InvalidChannelState(ChannelState),
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
    pub fn wouldblock(&self) -> bool {
        if let Error::IOError(e) = self {
            e.kind() == io::ErrorKind::WouldBlock
        } else {
            false
        }
    }

    pub fn interrupted(&self) -> bool {
        if let Error::IOError(e) = self {
            e.kind() == io::ErrorKind::Interrupted
        } else {
            false
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ChannelsLimitReached => write!(
                f,
                "the maximum number of channels for this connection has been reached"
            ),
            Error::InvalidProtocolVersion(version) => {
                write!(f, "the server only supports AMQP {}", version)
            }

            Error::InvalidChannel(channel) => write!(f, "invalid channel: {}", channel),
            Error::InvalidChannelState(state) => write!(f, "invalid channel state: {:?}", state),
            Error::InvalidConnectionState(state) => {
                write!(f, "invalid connection state: {:?}", state)
            }

            Error::IOError(e) => write!(f, "IO error: {}", e),
            Error::ParsingError(e) => write!(f, "failed to parse: {}", e),
            Error::ProtocolError(e) => write!(f, "protocol error: {}", e),
            Error::SerialisationError(e) => write!(f, "failed to serialise: {}", e),

            Error::MissingHeartbeatError => {
                write!(f, "no heartbeat received from server for too long")
            }

            Error::NoConfiguredExecutor => {
                write!(
                    f,
                    "an executor must be provided if the default-runtime feature is disabled"
                )
            }
            Error::NoConfiguredReactor => {
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
        match self {
            Error::IOError(e) => Some(&**e),
            Error::ParsingError(e) => Some(e),
            Error::ProtocolError(e) => Some(e),
            Error::SerialisationError(e) => Some(&**e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(other: io::Error) -> Self {
        Error::IOError(Arc::new(other))
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        use tracing::error;
        use Error::*;

        match (self, other) {
            (ChannelsLimitReached, ChannelsLimitReached) => true,
            (InvalidProtocolVersion(left_inner), InvalidProtocolVersion(right_version)) => {
                left_inner == right_version
            }

            (InvalidChannel(left_inner), InvalidChannel(right_inner)) => left_inner == right_inner,
            (InvalidChannelState(left_inner), InvalidChannelState(right_inner)) => {
                left_inner == right_inner
            }
            (InvalidConnectionState(left_inner), InvalidConnectionState(right_inner)) => {
                left_inner == right_inner
            }

            (IOError(_), IOError(_)) => {
                error!("Unable to compare lapin::Error::IOError");
                false
            }
            (ParsingError(left_inner), ParsingError(right_inner)) => left_inner == right_inner,
            (ProtocolError(left_inner), ProtocolError(right_inner)) => left_inner == right_inner,
            (SerialisationError(_), SerialisationError(_)) => {
                error!("Unable to compare lapin::Error::SerialisationError");
                false
            }

            _ => false,
        }
    }
}
