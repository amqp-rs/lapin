use api::ChannelState;

#[derive(Clone,Debug,PartialEq)]
pub enum Error {
  SendBufferTooSmall,
  ReceiveBufferTooSmall,
  GeneratorError,
  ParserError,
  InvalidState(InvalidState),
  InvalidMethod,
  InvalidChannel,
  NotConnected,
  UnexpectedAnswer,
}

#[derive(Clone,Debug,PartialEq)]
pub struct InvalidState {
    pub expected: ChannelState,
    pub actual:   ChannelState,
}
