use crate::message::BasicReturnMessage;

#[derive(Clone, Debug, PartialEq)]
pub enum PublisherConfirm {
    Ack,
    Nack(BasicReturnMessage),
    NotRequested,
}
