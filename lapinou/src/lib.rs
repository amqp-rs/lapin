use lapin::{executor::Executor, ConnectionProperties, Result};
use smol::Task;
use std::{future::Future, pin::Pin};

pub trait LapinSmolExt {
    fn with_smol(self) -> Self
    where
        Self: Sized;
}

impl LapinSmolExt for ConnectionProperties {
    fn with_smol(self) -> Self {
        self.with_executor(SmolExecutor)
    }
}

#[derive(Debug)]
struct SmolExecutor;

impl Executor for SmolExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        Task::spawn(f).detach();
        Ok(())
    }
}
