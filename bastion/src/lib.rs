use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};
use bastion::{spawn, blocking};

pub trait BastionExt {
    fn with_bastion(self) -> Self
        where
            Self: Sized;
}

impl BastionExt for ConnectionProperties {
    fn with_bastion(self) -> Self {
        self.with_executor(BastionExecutor)
    }
}

#[derive(Debug)]
struct BastionExecutor;

impl Executor for BastionExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        spawn!(f);
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        blocking!(f);
        Ok(())
    }
}
