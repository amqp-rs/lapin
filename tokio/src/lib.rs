use lapin::{executor::Executor, ConnectionProperties, Result};
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::runtime::Runtime;

pub trait LapinTokioExt {
    fn with_tokio(self, rt: Arc<Runtime>) -> Self
    where
        Self: Sized,
    {
        self.with_tokio_executor(rt)
    }

    fn with_tokio_executor(self, rt: Arc<Runtime>) -> Self
    where
        Self: Sized;
}

impl LapinTokioExt for ConnectionProperties {
    fn with_tokio_executor(self, rt: Arc<Runtime>) -> Self {
        self.with_executor(TokioExecutor(rt))
    }
}

#[derive(Debug)]
struct TokioExecutor(Arc<Runtime>);

impl Executor for TokioExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        self.0.spawn(f);
        Ok(())
    }
}
