use crate::{
    recovery_config::RecoveryConfig,
    reactor::FullReactor,
    types::{AMQPValue, FieldTable, LongString},
    ErrorKind, Result,
};
use executor_trait::FullExecutor;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn FullExecutor + Send + Sync>>,
    pub reactor: Option<Arc<dyn FullReactor + Send + Sync>>,
    pub recovery_config: Option<RecoveryConfig>,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            executor: None,
            reactor: None,
            recovery_config: None,
        }
    }
}

impl ConnectionProperties {
    #[must_use]
    pub fn with_connection_name(mut self, connection_name: LongString) -> Self {
        self.client_properties.insert(
            "connection_name".into(),
            AMQPValue::LongString(connection_name),
        );
        self
    }

    #[must_use]
    pub fn with_executor<E: FullExecutor + Send + Sync + 'static>(mut self, executor: E) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    #[must_use]
    pub fn with_reactor<R: FullReactor + Send + Sync + 'static>(mut self, reactor: R) -> Self {
        self.reactor = Some(Arc::new(reactor));
        self
    }

    #[must_use]
    pub fn with_experimental_recovery_config(mut self, config: RecoveryConfig) -> Self {
        self.recovery_config = Some(config);
        self
    }

    pub(crate) fn take_executor(&mut self) -> Result<Arc<dyn FullExecutor + Send + Sync>> {
        if let Some(executor) = self.executor.take() {
            return Ok(executor);
        }

        #[cfg(feature = "default-runtime")]
        {
            return Ok(Arc::new(async_global_executor_trait::AsyncGlobalExecutor));
        }

        #[allow(unreachable_code)]
        Err(ErrorKind::NoConfiguredExecutor.into())
    }

    pub(crate) fn take_reactor(&mut self) -> Result<Arc<dyn FullReactor + Send + Sync>> {
        if let Some(reactor) = self.reactor.take() {
            return Ok(reactor);
        }

        #[cfg(feature = "default-runtime")]
        {
            return Ok(Arc::new(async_reactor_trait::AsyncIo));
        }

        #[allow(unreachable_code)]
        Err(ErrorKind::NoConfiguredReactor.into())
    }
}
