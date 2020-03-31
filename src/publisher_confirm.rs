use crate::{
    message::BasicReturnMessage, pinky_swear::PinkySwear, returned_messages::ReturnedMessages,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

// FIXME: If dropped without data ready, register in ReturnedMessages
#[derive(Debug)]
pub struct PublisherConfirm {
    inner: PinkySwear<Confirmation>,
    returned_messages: ReturnedMessages,
    used: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Confirmation {
    Ack,
    Nack(BasicReturnMessage),
    NotRequested,
}

impl PublisherConfirm {
    pub(crate) fn new(
        inner: PinkySwear<Confirmation>,
        returned_messages: ReturnedMessages,
    ) -> Self {
        Self {
            inner,
            returned_messages,
            used: false,
        }
    }

    pub(crate) fn not_requested(returned_messages: ReturnedMessages) -> Self {
        Self {
            inner: PinkySwear::new_with_data(Confirmation::NotRequested),
            returned_messages,
            used: false,
        }
    }

    pub fn try_wait(&mut self) -> Option<Confirmation> {
        let confirmation = self.inner.try_wait()?;
        self.used = true;
        Some(confirmation)
    }

    pub fn wait(&mut self) -> Confirmation {
        self.used = true;
        self.inner.wait()
    }
}

impl Future for PublisherConfirm {
    type Output = Confirmation;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut();
        this.used = true;
        Pin::new(&mut this.inner).poll(cx)
    }
}
