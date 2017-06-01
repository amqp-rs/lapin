#[derive(Clone,Debug,PartialEq)]
pub enum Error {
  SendBufferTooSmall,
  ReceiveBufferTooSmall,
  GeneratorError,
  ParserError,
  InvalidState,
  InvalidMethod,
  InvalidChannel,
  NotConnected,
  UnexpectedAnswer,
}
