use lapin::ConnectionProperties;

pub trait LapinTokioExt {
    fn with_tokio(self) -> Self
    where
        Self: Sized,
    {
        let this = self.with_tokio_executor();
        #[cfg(unix)]
        let this = this.with_tokio_reactor();
        this
    }

    fn with_tokio_executor(self) -> Self
    where
        Self: Sized;

    #[cfg(unix)]
    fn with_tokio_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinTokioExt for ConnectionProperties {
    fn with_tokio_executor(self) -> Self {
        self.with_executor(tokio_executor_trait::Tokio::current())
    }

    #[cfg(unix)]
    fn with_tokio_reactor(self) -> Self {
        self.with_reactor(tokio_reactor_trait::Tokio)
    }
}
