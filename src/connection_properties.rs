use crate::{
    ErrorKind, Result,
    reactor::FullReactor,
    recovery_config::RecoveryConfig,
    types::{AMQPValue, FieldTable, LongString},
};
use backon::ExponentialBuilder;
use executor_trait::FullExecutor;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConnectionProperties {
    pub locale: String,
    pub client_properties: FieldTable,
    pub executor: Option<Arc<dyn FullExecutor + Send + Sync>>,
    pub reactor: Option<Arc<dyn FullReactor + Send + Sync>>,
    pub recovery_config: Option<RecoveryConfig>,
    pub backoff: ExponentialBuilder,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            executor: None,
            reactor: None,
            recovery_config: None,
            backoff: ExponentialBuilder::default().with_max_times(0 /* no retry by default */),
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
    #[cfg(feature = "unstable")]
    pub fn with_experimental_recovery_config(mut self, config: RecoveryConfig) -> Self {
        self.recovery_config = Some(config);
        self
    }

    #[must_use]
    pub fn with_backoff(mut self, backoff: ExponentialBuilder) -> Self {
        self.backoff = backoff;
        self
    }

    #[must_use]
    pub fn configure_backoff(mut self, conf: impl Fn(&mut ExponentialBuilder)) -> Self {
        conf(&mut self.backoff);
        self
    }

    pub(crate) fn executor(&self) -> Result<Arc<dyn FullExecutor + Send + Sync>> {
        if let Some(executor) = self.executor.clone() {
            return Ok(executor);
        }

        #[cfg(feature = "default-runtime")]
        {
            return Ok(Arc::new(async_global_executor_trait::AsyncGlobalExecutor));
        }

        #[allow(unreachable_code)]
        Err(ErrorKind::NoConfiguredExecutor.into())
    }

    pub(crate) fn reactor(&self) -> Result<Arc<dyn FullReactor + Send + Sync>> {
        if let Some(reactor) = self.reactor.clone() {
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
