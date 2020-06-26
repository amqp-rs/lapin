use lapin::{executor::Executor, ConnectionProperties, Result};
use lapinou::LapinSmolExt;
use std::{future::Future, pin::Pin};

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
        // async-std uses smol underneath, use smol reactor until async-std exposes its own API
        self.with_smol_reactor()
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
