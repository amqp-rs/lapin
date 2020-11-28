use crate::{Result, TcpStream};
use async_io::{Async, Timer};
use async_trait::async_trait;
use futures_lite::io::{AsyncRead, AsyncWrite};
use std::{fmt, time::Duration};

pub trait AsyncRW: AsyncRead + AsyncWrite {}
impl<IO: AsyncRead + AsyncWrite> AsyncRW for IO {}

#[async_trait]
pub trait Reactor: fmt::Debug + Send {
    fn register(&self, socket: TcpStream) -> Result<Box<dyn AsyncRW + Send>>;
    async fn sleep(&self, dur: Duration);
}

#[derive(Debug)]
pub(crate) struct DefaultReactor;

#[async_trait]
impl Reactor for DefaultReactor {
    fn register(&self, socket: TcpStream) -> Result<Box<dyn AsyncRW + Send>> {
        Ok(Box::new(Async::new(socket)?))
    }

    async fn sleep(&self, dur: Duration) {
        Timer::after(dur).await;
    }
}
