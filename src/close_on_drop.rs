use crate::{pinky_swear::PinkySwear, types::ShortUInt, Result};
use amq_protocol::protocol;
use log::error;
use std::ops::Deref;

#[derive(Debug)]
pub struct CloseOnDrop<T: __private::Closable> {
    inner: Option<T>,
}

impl<T: __private::Closable> CloseOnDrop<T> {
    pub fn new(inner: T) -> Self {
        Self { inner: Some(inner) }
    }

    /// Give the underlying object, cancelling the close on drop action
    pub fn into_inner(mut self) -> T {
        let inner = self
            .inner
            .take()
            .expect("inner should only be None once consumed or dropped");
        std::mem::forget(self);
        inner
    }

    pub fn close(self, reply_code: ShortUInt, reply_text: &str) -> PinkySwear<Result<()>> {
        self.into_inner().close(reply_code, reply_text)
    }
}

impl<T: __private::Closable> Deref for CloseOnDrop<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
            .as_ref()
            .expect("inner should only be None once consumed or dropped")
    }
}

impl<T: __private::Closable> Drop for CloseOnDrop<T> {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.as_ref() {
            if let Err(err) = inner
                .close(protocol::constants::REPLY_SUCCESS.into(), "OK")
                .wait()
            {
                error!("Failed to close on drop: {:?}", err);
            }
        }
    }
}

pub(crate) mod __private {
    use super::*;

    pub trait Closable {
        fn close(&self, reply_code: ShortUInt, reply_text: &str) -> PinkySwear<Result<()>>;
    }
}
