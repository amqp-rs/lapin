use async_lapin::LapinAsyncIoExt;
use lapin::ConnectionProperties;

// ConnectionProperties extension

#[deprecated(note = "use smol-executor-trait and async-reactor-trait directly instead")]
pub trait LapinSmolExt {
    #[deprecated(note = "use smol-executor-trait directly instead")]
    fn with_smol(self) -> Self
    where
        Self: Sized,
    {
        self.with_smol_executor().with_smol_reactor()
    }

    #[deprecated(note = "use smol-executor-trait directly instead")]
    fn with_smol_executor(self) -> Self
    where
        Self: Sized;

    #[deprecated(note = "use async-reactor-trait directly instead")]
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
