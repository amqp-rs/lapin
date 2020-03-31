#[derive(Clone, Debug, PartialEq)]
pub enum PublisherConfirm {
    Ack,
    Nack,
    NotRequested,
}
