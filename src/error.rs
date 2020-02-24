use crate::{channel_status::ChannelState, connection_status::ConnectionState};
use amq_protocol::{frame::GenError, protocol::{AMQPClass, AMQPError}};
use std::{error, fmt, io, sync::Arc};

/// A std Result with a lapin::Error error type
pub type Result<T> = std::result::Result<T, Error>;

/// The type of error that can be returned in this crate.
///
/// Even though we expose the complete enumeration of possible error variants, it is not
/// considered stable to exhaustively match on this enumeration: do it at your own risk.
#[derive(Clone, Debug)]
pub enum Error {
    InvalidMethod(AMQPClass),
    InvalidChannel(u16),
    ConnectionRefused,
    NotConnected,
    UnexpectedReply,
    PreconditionFailed,
    ChannelLimitReached,
    InvalidBodyReceived,
    InvalidChannelState(ChannelState),
    InvalidConnectionState(ConnectionState),
    ParsingError(String),
    ProtocolError(AMQPError, String),
    SerialisationError(Arc<GenError>),
    IOError(Arc<io::Error>),
    /// A hack to prevent developers from exhaustively match on the enum's variants
    ///
    /// The purpose of this variant is to let the `Error` enumeration grow more variants
    /// without it being a breaking change for users. It is planned for the language to provide
    /// this functionnality out of the box, though it has not been [stabilized] yet.
    ///
    /// [stabilized]: https://github.com/rust-lang/rust/issues/44109
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Error {
    pub fn wouldblock(&self) -> bool {
        if let Error::IOError(e) = self {
            e.kind() == io::ErrorKind::WouldBlock
        } else {
            false
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidMethod(method) => write!(f, "invalid protocol method: {:?}", method),
            Error::InvalidChannel(channel) => write!(f, "invalid channel: {}", channel),
            Error::ConnectionRefused => write!(f, "connection refused"),
            Error::NotConnected => write!(f, "not connected"),
            Error::UnexpectedReply => write!(f, "unexpected reply"),
            Error::PreconditionFailed => write!(f, "precondition failed"),
            Error::ChannelLimitReached => write!(
                f,
                "The maximum number of channels for this connection has been reached"
            ),
            Error::InvalidBodyReceived => write!(f, "invalid body received"),
            Error::InvalidChannelState(state) => write!(f, "invalid channel state: {:?}", state),
            Error::InvalidConnectionState(state) => {
                write!(f, "invalid connection state: {:?}", state)
            }
            Error::ParsingError(e) => write!(f, "Failed to parse: {}", e),
            Error::ProtocolError(e, msg) => write!(f, "Protocol error: {:?} - {}", e, msg),
            Error::SerialisationError(e) => write!(f, "Failed to serialise: {:?}", e),
            Error::IOError(e) => write!(f, "IO error: {:?}", e),
            Error::__Nonexhaustive => write!(
                f,
                "lapin::Error::__Nonexhaustive: this should not be printed"
            ),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::SerialisationError(e) => Some(&**e),
            Error::IOError(e) => Some(&**e),
            _ => None,
        }
    }
}

#[cfg(test)]
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        use Error::*;

        match (self, other) {
            (InvalidMethod(left_inner), InvalidMethod(right_inner)) => left_inner == right_inner,
            (InvalidChannel(left_inner), InvalidChannel(right_inner)) => left_inner == right_inner,
            (ParsingError(left_inner), ParsingError(right_inner)) => left_inner == right_inner,
            (InvalidChannelState(left_inner), InvalidChannelState(right_inner)) => {
                left_inner == right_inner
            }
            (InvalidConnectionState(left_inner), InvalidConnectionState(right_inner)) => {
                left_inner == right_inner
            }

            (ConnectionRefused, ConnectionRefused) => true,
            (NotConnected, NotConnected) => true,
            (UnexpectedReply, UnexpectedReply) => true,
            (PreconditionFailed, PreconditionFailed) => true,
            (ChannelLimitReached, ChannelLimitReached) => true,

            (SerialisationError(_), SerialisationError(_)) => {
                panic!("Unable to compare lapin::Error::SerialisationError");
            }

            (IOError(_), IOError(_)) => {
                panic!("Unable to compare lapin::Error::IOError");
            }

            (__Nonexhaustive, __Nonexhaustive) => {
                panic!("lapin::Error::__Nonexhaustive: should not be compared");
            }

            _ => false,
        }
    }
}
