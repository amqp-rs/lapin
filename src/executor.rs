use crate::Result;
use std::{fmt, future::Future, ops::Deref, pin::Pin, sync::Arc};

pub trait Executor: std::fmt::Debug + Send + Sync {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>);
    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>);
}

impl Executor for Arc<dyn Executor> {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.deref().spawn(f);
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) {
        self.deref().spawn_blocking(f)
    }
}

#[derive(Clone)]
pub struct DefaultExecutor;

impl DefaultExecutor {
    pub(crate) fn default() -> Result<Arc<dyn Executor>> {
        Ok(Arc::new(Self))
    }
}

impl fmt::Debug for DefaultExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultExecutor").finish()
    }
}

impl Executor for DefaultExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        async_global_executor::spawn(f).detach();
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) {
        async_global_executor::spawn(blocking::unblock(f)).detach();
    }
}
