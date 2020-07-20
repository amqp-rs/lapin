use async_lapin::*;
use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

// ConnectionProperties extension

pub trait LapinAsyncStdExt {
    fn with_async_std(self) -> Self
    where
        Self: Sized,
    {
        self.with_async_std_executor().with_async_std_reactor()
    }

    fn with_async_std_executor(self) -> Self
    where
        Self: Sized;

    fn with_async_std_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncStdExt for ConnectionProperties {
    fn with_async_std(self) -> Self {
        self.with_async_std_executor().with_async_std_reactor()
    }

    fn with_async_std_executor(self) -> Self {
        self.with_executor(AsyncStdExecutor)
    }

    fn with_async_std_reactor(self) -> Self {
        // async-std uses async-io underneath, use async-io reactor until async-std exposes its own API
        self.with_async_io_reactor(|fut| drop(async_std::task::spawn(fut)))
    }
}

// Executor

#[derive(Debug)]
struct AsyncStdExecutor;

impl Executor for AsyncStdExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        async_std::task::spawn(f);
        Ok(())
    }
}
