use crate::{
    message::BasicReturnMessage,
    pinky_swear::PinkySwear,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Debug)]
pub struct PublisherConfirm {
    inner: PinkySwear<Confirmation>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Confirmation {
    Ack,
    Nack(BasicReturnMessage),
    NotRequested,
}

impl PublisherConfirm {
    pub fn not_requested() -> Self {
        Self {
            inner: PinkySwear::new_with_data(Confirmation::NotRequested),
        }
    }

    pub fn try_wait(&self) -> Option<Confirmation> {
        self.inner.try_wait()
    }

    pub fn wait(&self) -> Confirmation {
        self.inner.wait()
    }

    pub fn traverse<F: Send + 'static>(
        self,
        transform: Box<dyn Fn(Confirmation) -> F + Send>,
    ) -> PinkySwear<F, Confirmation> {
        self.inner.traverse(transform)
    }
}

impl From<PinkySwear<Confirmation>> for PublisherConfirm {
    fn from(inner: PinkySwear<Confirmation>) -> Self {
        Self { inner }
    }
}

impl Future for PublisherConfirm {
    type Output = Confirmation;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.as_mut().inner).poll(cx)
    }
}
