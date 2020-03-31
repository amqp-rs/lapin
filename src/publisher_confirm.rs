use crate::message::BasicReturnMessage;

#[derive(Clone, Debug, PartialEq)]
pub enum Confirmation {
    Ack,
    Nack(BasicReturnMessage),
    NotRequested,
}
