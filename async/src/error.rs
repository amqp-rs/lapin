#[derive(Clone,Debug,PartialEq)]
pub enum Error {
  SendBufferTooSmall,
  ReceiveBufferTooSmall,
  GeneratorError,
  ParserError,
  InvalidState,
  InvalidChannel,
}
