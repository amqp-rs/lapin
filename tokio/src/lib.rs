use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

pub trait LapinTokioExt {
    fn with_tokio(self) -> Self
    where
        Self: Sized;
}

impl LapinTokioExt for ConnectionProperties {
    fn with_tokio(self) -> Self {
        self.with_executor(TokioExecutor)
    }
}

#[derive(Debug)]
struct TokioExecutor;

impl Executor for TokioExecutor {
    fn execute(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        tokio::spawn(f);
        Ok(())
    }
}
