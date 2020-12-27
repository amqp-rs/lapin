use lapin::ConnectionProperties;

// ConnectionProperties extension

pub trait LapinAsyncGlobalExecutorExt {
    fn with_async_global_executor(self) -> Self
    where
        Self: Sized;

    #[cfg(feature = "async-io")]
    fn with_async_io(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncGlobalExecutorExt for ConnectionProperties {
    fn with_async_global_executor(self) -> Self {
        self.with_executor(async_global_executor_trait::AsyncGlobalExecutor)
    }

    #[cfg(feature = "async-io")]
    fn with_async_io(self) -> Self {
        async_lapin::LapinAsyncIoExt::with_async_io(self)
    }
}
