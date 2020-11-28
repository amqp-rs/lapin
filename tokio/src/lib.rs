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
        os::unix::io::{AsRawFd, RawFd},
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
            let raw_fd = socket.as_raw_fd();
            Ok(Box::new(AsyncFdWrapper {
                fd: AsyncFd::new(raw_fd)?,
                socket,
                readable: false,
                writable: false,
            }))
        }

        async fn sleep(&self, dur: Duration) {
            tokio::time::sleep(dur).await;
        }
    }

    struct AsyncFdWrapper {
        fd: AsyncFd<RawFd>,
        socket: TcpStream,
        readable: bool,
        writable: bool,
    }

    impl AsyncFdWrapper {
        fn read<F: FnOnce(&mut TcpStream) -> futures_io::Result<usize>>(
            &mut self,
            cx: &mut Context<'_>,
            f: F,
        ) -> Poll<futures_io::Result<usize>> {
            if !self.readable {
                match self.fd.poll_read_ready(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Ready(Ok(mut guard)) => guard.clear_ready(),
                };
                self.readable = true;
            }
            // We should use guard.with_io above, but cannot because we need &mut and self is
            // already borrowed
            Poll::Ready(match f(&mut self.socket) {
                Err(e) if e.kind() == futures_io::ErrorKind::WouldBlock => {
                    self.readable = false;
                    Err(e)
                }
                other => other,
            })
        }

        fn write<R, F: FnOnce(&mut TcpStream) -> futures_io::Result<R>>(
            &mut self,
            cx: &mut Context<'_>,
            f: F,
        ) -> Poll<futures_io::Result<R>> {
            if !self.writable {
                match self.fd.poll_write_ready(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Ready(Ok(mut guard)) => guard.retain_ready(),
                };
                self.writable = true;
            }
            Poll::Ready(match f(&mut self.socket) {
                Err(e) if e.kind() == futures_io::ErrorKind::WouldBlock => {
                    self.writable = false;
                    Err(e)
                }
                other => other,
            })
        }
    }

    impl AsyncRead for AsyncFdWrapper {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<futures_io::Result<usize>> {
            self.read(cx, |socket| socket.read(buf))
        }

        fn poll_read_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &mut [IoSliceMut<'_>],
        ) -> Poll<futures_io::Result<usize>> {
            self.read(cx, |socket| socket.read_vectored(bufs))
        }
    }

    impl AsyncWrite for AsyncFdWrapper {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<futures_io::Result<usize>> {
            self.write(cx, |socket| socket.write(buf))
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<futures_io::Result<usize>> {
            self.write(cx, |socket| socket.write_vectored(bufs))
        }

        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<futures_io::Result<()>> {
            self.write(cx, |socket| socket.flush())
        }

        fn poll_close(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<futures_io::Result<()>> {
            self.poll_flush(cx)
        }
    }
}
