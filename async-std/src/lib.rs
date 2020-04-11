use lapin::{executor::Executor, ConnectionProperties, Result};
use std::sync::Arc;

pub trait LapinAsyncStdExt {
    fn with_async_std(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncStdExt for ConnectionProperties {
    fn with_async_std(mut self) -> Self {
        self.executor = Some(Arc::new(AsyncStdExecutor));
        self
    }
}

#[derive(Debug)]
struct AsyncStdExecutor;

impl Executor for AsyncStdExecutor {
    fn execute(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        async_std::task::spawn(async {
            f();
        });
        Ok(())
    }
}
