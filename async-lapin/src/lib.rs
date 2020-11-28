use async_io::{Async, Timer};
use async_trait::async_trait;
use lapin::{
    reactor::{AsyncRW, Reactor},
    ConnectionProperties, Result, TcpStream,
};
use std::time::Duration;

// ConnectionProperties extension

pub trait LapinAsyncIoExt {
    fn with_async_io(self) -> Self
    where
        Self: Sized,
    {
        self.with_async_io_reactor()
    }

    fn with_async_io_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncIoExt for ConnectionProperties {
    fn with_async_io_reactor(self) -> Self {
        self.with_reactor(AsyncIoReactor)
    }
}

// Reactor

#[derive(Debug)]
struct AsyncIoReactor;

#[async_trait]
impl Reactor for AsyncIoReactor {
    fn register(&self, socket: TcpStream) -> Result<Box<dyn AsyncRW + Send>> {
        Ok(Box::new(Async::new(socket)?))
    }

    async fn sleep(&self, dur: Duration) {
        Timer::after(dur).await;
    }
}
