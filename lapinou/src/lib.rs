use async_lapin::LapinAsyncIoExt;
use lapin::ConnectionProperties;

// ConnectionProperties extension

pub trait LapinSmolExt {
    fn with_smol(self) -> Self
    where
        Self: Sized,
    {
        self.with_smol_executor().with_smol_reactor()
    }

    fn with_smol_executor(self) -> Self
    where
        Self: Sized;

    fn with_smol_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinSmolExt for ConnectionProperties {
    fn with_smol_executor(self) -> Self {
        self.with_executor(smol_executor_trait::Smol)
    }

    fn with_smol_reactor(self) -> Self {
        self.with_async_io_reactor()
    }
}
