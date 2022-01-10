use lapin::ConnectionProperties;

// ConnectionProperties extension

#[deprecated(note = "use async-reactor-trait directly instead")]
pub trait LapinAsyncIoExt {
    #[deprecated(note = "use async-reactor-trait directly instead")]
    fn with_async_io(self) -> Self
    where
        Self: Sized,
    {
        self.with_async_io_reactor()
    }

    #[deprecated(note = "use async-reactor-trait directly instead")]
    fn with_async_io_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncIoExt for ConnectionProperties {
    fn with_async_io_reactor(self) -> Self {
        self.with_reactor(async_reactor_trait::AsyncIo)
    }
}
