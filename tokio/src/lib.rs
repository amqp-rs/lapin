use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};
use tokio::runtime::Handle;

pub trait LapinTokioExt {
    fn with_tokio(self) -> Self
    where
        Self: Sized,
    {
        self.with_tokio_executor()
    }

    fn with_tokio_executor(self) -> Self
    where
        Self: Sized;
}

impl LapinTokioExt for ConnectionProperties {
    fn with_tokio_executor(self) -> Self {
        self.with_executor(TokioExecutor(Handle::current()))
    }
}

#[derive(Debug)]
struct TokioExecutor(Handle);

impl Executor for TokioExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.0.spawn(f);
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        self.0.spawn_blocking(f);
        Ok(())
    }
}
