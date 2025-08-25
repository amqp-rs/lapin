use crate::Result;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) struct InternalFuture(
    pub(crate) Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>,
);

impl Future for InternalFuture {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl fmt::Debug for InternalFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InternalFuture").finish()
    }
}
