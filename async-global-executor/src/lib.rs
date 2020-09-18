use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

// ConnectionProperties extension

pub trait LapinAsyncGlobalExecutorExt {
    fn with_async_global_executor(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncGlobalExecutorExt for ConnectionProperties {
    fn with_async_global_executor(self) -> Self {
        self.with_executor(AsyncGlobalExecutorExecutor)
    }
}

// Executor

#[derive(Debug)]
struct AsyncGlobalExecutorExecutor;

impl Executor for AsyncGlobalExecutorExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        async_global_executor::spawn(f).detach();
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        async_global_executor::spawn(blocking::unblock(f)).detach();
        Ok(())
    }
}
