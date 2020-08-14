use async_executor::Spawner;
use async_lapin::*;
use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

// ConnectionProperties extension

pub trait LapinSmolExt {
    fn with_smol(self) -> Self
    where
        Self: Sized,
    {
        self.with_smol_executor().with_smol_reactor()
    }

    fn with_smol_executor(self) -> Self
    where
        Self: Sized;

    fn with_smol_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinSmolExt for ConnectionProperties {
    fn with_smol_executor(self) -> Self {
        self.with_executor(SmolExecutor(Spawner::current()))
    }

    fn with_smol_reactor(self) -> Self {
        self.with_async_io_reactor(SmolExecutor(Spawner::current()))
    }
}

// Executor

#[derive(Debug)]
struct SmolExecutor(Spawner);

impl Executor for SmolExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.0.spawn(f).detach();
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        self.0.spawn(blocking::unblock(f)).detach();
        Ok(())
    }
}
