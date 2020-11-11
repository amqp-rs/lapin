use crate::{message::BasicReturnMessage, returned_messages::ReturnedMessages, Promise, Result};
use log::trace;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub struct PublisherConfirm {
    inner: Option<Promise<Confirmation>>,
    returned_messages: ReturnedMessages,
    used: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Confirmation {
    Ack(Option<Box<BasicReturnMessage>>),
    Nack(Option<Box<BasicReturnMessage>>),
    NotRequested,
}

impl Confirmation {
    pub fn take_message(self) -> Option<BasicReturnMessage> {
        if let Confirmation::Ack(Some(msg)) | Confirmation::Nack(Some(msg)) = self {
            Some(*msg)
        } else {
            None
        }
    }

    pub fn is_ack(&self) -> bool {
        matches!(self, Confirmation::Ack(_))
    }

    pub fn is_nack(&self) -> bool {
        matches!(self, Confirmation::Nack(_))
    }
}

impl PublisherConfirm {
    pub(crate) fn new(inner: Promise<Confirmation>, returned_messages: ReturnedMessages) -> Self {
        Self {
            inner: Some(inner),
            returned_messages,
            used: false,
        }
    }

    pub(crate) fn not_requested(returned_messages: ReturnedMessages) -> Self {
        Self {
            inner: Some(Promise::new_with_data(Ok(Confirmation::NotRequested))),
            returned_messages,
            used: false,
        }
    }

    pub fn try_wait(&mut self) -> Option<Result<Confirmation>> {
        let confirmation = self
            .inner
            .as_ref()
            .expect("inner should only be None after Drop")
            .try_wait()?;
        self.used = true;
        Some(confirmation)
    }

    pub fn wait(&mut self) -> Result<Confirmation> {
        self.used = true;
        self.inner
            .as_ref()
            .expect("inner should only be None after Drop")
            .wait()
    }
}

impl fmt::Debug for PublisherConfirm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublisherConfirm").finish()
    }
}

impl Future for PublisherConfirm {
    type Output = Result<Confirmation>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut();
        let res = Pin::new(
            &mut this
                .inner
                .as_mut()
                .expect("inner should only be None after Drop"),
        )
        .poll(cx);
        if res.is_ready() {
            this.used = true;
        }
        res
    }
}

impl Drop for PublisherConfirm {
    fn drop(&mut self) {
        if !self.used {
            if let Some(promise) = self.inner.take() {
                trace!(
                    "PublisherConfirm dropped without use, registering it for wait_for_confirms"
                );
                self.returned_messages.register_dropped_confirm(promise);
            }
        }
    }
}
