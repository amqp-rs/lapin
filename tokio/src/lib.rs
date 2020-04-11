use lapin::{executor::Executor, ConnectionProperties, Result};
use std::sync::Arc;

pub trait LapinTokioExt {
    fn with_tokio(self) -> Self
    where
        Self: Sized;
}

impl LapinTokioExt for ConnectionProperties {
    fn with_tokio(mut self) -> Self {
        self.executor = Some(Arc::new(TokioExecutor));
        self
    }
}

#[derive(Debug)]
struct TokioExecutor;

impl Executor for TokioExecutor {
    fn execute(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        tokio::spawn(async { f() });
        Ok(())
    }
}
