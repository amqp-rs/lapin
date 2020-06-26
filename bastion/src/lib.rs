use bastion::spawn;
use lapin::{executor::Executor, ConnectionProperties};
use std::{future::Future, pin::Pin};

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
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<(), lapin::Error> {
        spawn!(f);
        Ok(())
    }
}
