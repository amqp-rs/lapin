use crate::{ErrorKind, Result, reactor::FullReactor};
use executor_trait::FullExecutor;
use std::sync::{Arc, Mutex};

static EXECUTOR: Mutex<Option<Arc<dyn FullExecutor + Send + Sync + 'static>>> = Mutex::new(None);
static REACTOR: Mutex<Option<Arc<dyn FullReactor + Send + Sync + 'static>>> = Mutex::new(None);

pub fn install_runtime<
    E: FullExecutor + Send + Sync + 'static,
    R: FullReactor + Send + Sync + 'static,
>(
    executor: E,
    reactor: R,
) {
    install_executor(executor);
    install_reactor(reactor);
}

pub fn install_executor<E: FullExecutor + Send + Sync + 'static>(executor: E) {
    *EXECUTOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(executor));
}

pub fn install_reactor<R: FullReactor + Send + Sync + 'static>(reactor: R) {
    *REACTOR.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(reactor));
}

pub(crate) fn executor() -> Result<Arc<dyn FullExecutor + Send + Sync + 'static>> {
    if let Some(executor) = EXECUTOR.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(executor);
    }

    #[cfg(feature = "default-runtime")]
    {
        return Ok(Arc::new(tokio_executor_trait::Tokio::current()));
    }

    #[allow(unreachable_code)]
    Err(ErrorKind::NoConfiguredExecutor.into())
}

pub(crate) fn reactor() -> Result<Arc<dyn FullReactor + Send + Sync + 'static>> {
    if let Some(reactor) = REACTOR.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(reactor);
    }

    #[cfg(feature = "default-runtime")]
    {
        return Ok(Arc::new(tokio_reactor_trait::Tokio::current()));
    }

    #[allow(unreachable_code)]
    Err(ErrorKind::NoConfiguredReactor.into())
}
