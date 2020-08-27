use async_lapin::*;
use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

// ConnectionProperties extension

pub trait LapinAsyncGlobalExecutorExt {
    fn with_async_global_executor(self) -> Self
    where
        Self: Sized,
    {
        self.with_async_global_executor_executor()
            .with_async_global_executor_reactor()
    }

    fn with_async_global_executor_executor(self) -> Self
    where
        Self: Sized;

    fn with_async_global_executor_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncGlobalExecutorExt for ConnectionProperties {
    fn with_async_global_executor_executor(self) -> Self {
        self.with_executor(AsyncGlobalExecutorExecutor)
    }

    fn with_async_global_executor_reactor(self) -> Self {
        self.with_async_io_reactor(AsyncGlobalExecutorExecutor)
    }
}

// Executor

#[derive(Debug)]
struct AsyncGlobalExecutorExecutor;

impl Executor for AsyncGlobalExecutorExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        async_global_executor::spawn(f).detach();
        Ok(())
    }
}
