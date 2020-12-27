use lapin::ConnectionProperties;

// ConnectionProperties extension

pub trait LapinAsyncIoExt {
    fn with_async_io(self) -> Self
    where
        Self: Sized,
    {
        self.with_async_io_reactor()
    }

    fn with_async_io_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinAsyncIoExt for ConnectionProperties {
    fn with_async_io_reactor(self) -> Self {
        self.with_reactor(async_reactor_trait::AsyncIo)
    }
}
