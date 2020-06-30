use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

pub trait BastionExt {
    fn with_bastion(self) -> Self
    where
        Self: Sized,
    {
        self.with_bastion_executor()
    }

    fn with_bastion_executor(self) -> Self
    where
        Self: Sized;
}

impl BastionExt for ConnectionProperties {
    fn with_bastion_executor(self) -> Self {
        self.with_executor(BastionExecutor)
    }
}

#[derive(Debug)]
struct BastionExecutor;

impl Executor for BastionExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        bastion_executor::pool::spawn(f, lightproc::proc_stack::ProcStack::default());
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        bastion_executor::blocking::spawn_blocking(async move { f() }, lightproc::proc_stack::ProcStack::default());
        Ok(())
    }
}
