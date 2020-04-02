use crate::{channel_status::ChannelState, connection_status::ConnectionState};
use amq_protocol::{
    frame::{GenError, ParserError},
    protocol::{AMQPClass, AMQPError},
};
use std::{error, fmt, io, sync::Arc};

/// A std Result with a lapin::Error error type
pub type Result<T> = std::result::Result<T, Error>;

/// The type of error that can be returned in this crate.
///
/// Even though we expose the complete enumeration of possible error variants, it is not
/// considered stable to exhaustively match on this enumeration: do it at your own risk.
#[derive(Clone, Debug)]
pub enum Error {
    ChannelsLimitReached,
    InvalidAck,
    InvalidBodyReceived,
    InvalidFrameReceived,
    UnexpectedReply,

    InvalidChannel(u16),
    InvalidChannelState(ChannelState),
    InvalidConnectionState(ConnectionState),
    InvalidMethod(AMQPClass),

    IOError(Arc<io::Error>),
    ParsingError(ParserError),
    ProtocolError(AMQPError),
    SerialisationError(Arc<GenError>),

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
            Error::ChannelsLimitReached => write!(
                f,
                "the maximum number of channels for this connection has been reached"
            ),
            Error::InvalidAck => write!(f, "invalid acknowledgement"),
            Error::InvalidBodyReceived => write!(f, "invalid body received"),
            Error::InvalidFrameReceived => write!(f, "invalid frame received"),
            Error::UnexpectedReply => write!(f, "unexpected reply"),

            Error::InvalidChannel(channel) => write!(f, "invalid channel: {}", channel),
            Error::InvalidChannelState(state) => write!(f, "invalid channel state: {:?}", state),
            Error::InvalidConnectionState(state) => {
                write!(f, "invalid connection state: {:?}", state)
            }
            Error::InvalidMethod(method) => write!(f, "invalid protocol method: {:?}", method),

            Error::IOError(e) => write!(f, "IO error: {}", e),
            Error::ParsingError(e) => write!(f, "failed to parse: {}", e),
            Error::ProtocolError(e) => write!(f, "protocol error: {}", e),
            Error::SerialisationError(e) => write!(f, "failed to serialise: {}", e),

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
            Error::IOError(e) => Some(&**e),
            Error::ParsingError(e) => Some(&*e),
            Error::ProtocolError(e) => Some(&*e),
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
        use log::error;
        use Error::*;

        match (self, other) {
            (ChannelsLimitReached, ChannelsLimitReached) => true,
            (InvalidAck, InvalidAck) => true,
            (InvalidBodyReceived, InvalidBodyReceived) => true,
            (InvalidFrameReceived, InvalidFrameReceived) => true,
            (UnexpectedReply, UnexpectedReply) => true,

            (InvalidChannel(left_inner), InvalidChannel(right_inner)) => left_inner == right_inner,
            (InvalidChannelState(left_inner), InvalidChannelState(right_inner)) => {
                left_inner == right_inner
            }
            (InvalidConnectionState(left_inner), InvalidConnectionState(right_inner)) => {
                left_inner == right_inner
            }
            (InvalidMethod(left_inner), InvalidMethod(right_inner)) => left_inner == right_inner,

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
