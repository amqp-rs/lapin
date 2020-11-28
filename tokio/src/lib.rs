use lapin::{executor::Executor, ConnectionProperties};
use std::{future::Future, pin::Pin};
use tokio::runtime::Handle;

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
        self.with_executor(TokioExecutor(Handle::current()))
    }

    #[cfg(unix)]
    fn with_tokio_reactor(self) -> Self {
        self.with_reactor(unix::TokioReactor(Handle::current()))
    }
}

#[derive(Debug)]
struct TokioExecutor(Handle);

impl Executor for TokioExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.0.spawn(f);
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) {
        self.0.spawn_blocking(f);
    }
}

#[cfg(unix)]
mod unix {
    use async_trait::async_trait;
    use lapin::{
        reactor::{AsyncRW, Reactor},
        Result, TcpStream,
    };
    use std::time::Duration;
    use tokio::{io::unix::AsyncFd, runtime::Handle};

    #[derive(Debug)]
    pub(super) struct TokioReactor(pub(super) Handle);

    #[async_trait]
    impl Reactor for TokioReactor {
        fn register(&self, socket: TcpStream) -> Result<Box<dyn AsyncRW + Send>> {
            Ok(Box::new(AsyncFd::new(socket)?))
        }

        async fn sleep(&self, dur: Duration) {
            tokio::time::sleep(dur).await;
        }
    }
}
