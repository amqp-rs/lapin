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
    use futures_io::{AsyncRead, AsyncWrite};
    use lapin::{
        reactor::{AsyncRW, Reactor},
        Result, TcpStream,
    };
    use std::{
        io::{IoSlice, IoSliceMut, Read, Write},
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::{io::unix::AsyncFd, runtime::Handle};

    #[derive(Debug)]
    pub(super) struct TokioReactor(pub(super) Handle);

    #[async_trait]
    impl Reactor for TokioReactor {
        fn register(&self, socket: TcpStream) -> Result<Box<dyn AsyncRW + Send>> {
            Ok(Box::new(AsyncFdWrapper(AsyncFd::new(socket)?)))
        }

        async fn sleep(&self, dur: Duration) {
            tokio::time::sleep(dur).await;
        }
    }

    struct AsyncFdWrapper(AsyncFd<TcpStream>);

    impl AsyncFdWrapper {
        fn read<F: FnOnce(&mut AsyncFd<TcpStream>) -> futures_io::Result<usize>>(
            &mut self,
            cx: &mut Context<'_>,
            f: F,
        ) -> Option<Poll<futures_io::Result<usize>>> {
            Some(match self.0.poll_read_ready_mut(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(mut guard)) => match guard.try_io(f) {
                    Ok(res) => Poll::Ready(res),
                    Err(_) => return None,
                },
            })
        }

        fn write<R, F: FnOnce(&mut AsyncFd<TcpStream>) -> futures_io::Result<R>>(
            &mut self,
            cx: &mut Context<'_>,
            f: F,
        ) -> Option<Poll<futures_io::Result<R>>> {
            Some(match self.0.poll_write_ready_mut(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Ready(Ok(mut guard)) => match guard.try_io(f) {
                    Ok(res) => Poll::Ready(res),
                    Err(_) => return None,
                },
            })
        }
    }

    impl AsyncRead for AsyncFdWrapper {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<futures_io::Result<usize>> {
            loop {
                if let Some(res) = self.read(cx, |socket| socket.get_mut().read(buf)) {
                    return res;
                }
            }
        }

        fn poll_read_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &mut [IoSliceMut<'_>],
        ) -> Poll<futures_io::Result<usize>> {
            loop {
                if let Some(res) = self.read(cx, |socket| socket.get_mut().read_vectored(bufs)) {
                    return res;
                }
            }
        }
    }

    impl AsyncWrite for AsyncFdWrapper {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<futures_io::Result<usize>> {
            loop {
                if let Some(res) = self.write(cx, |socket| socket.get_mut().write(buf)) {
                    return res;
                }
            }
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<futures_io::Result<usize>> {
            loop {
                if let Some(res) = self.write(cx, |socket| socket.get_mut().write_vectored(bufs)) {
                    return res;
                }
            }
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<futures_io::Result<()>> {
            loop {
                if let Some(res) = self.write(cx, |socket| socket.get_mut().flush()) {
                    return res;
                }
            }
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<futures_io::Result<()>> {
            self.poll_flush(cx)
        }
    }
}
