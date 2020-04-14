use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

pub trait LapinAsyncStdExt {
    fn with_async_std(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncStdExt for ConnectionProperties {
    fn with_async_std(self) -> Self {
        self.with_executor(AsyncStdExecutor)
    }
}

#[derive(Debug)]
struct AsyncStdExecutor;

impl Executor for AsyncStdExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        async_std::task::spawn(f);
        Ok(())
    }
}
