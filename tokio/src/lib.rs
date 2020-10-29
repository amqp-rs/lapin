use lapin::{executor::Executor, ConnectionProperties};
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::runtime::Runtime;

pub trait LapinTokioExt {
    fn with_tokio(self, rt: Arc<Runtime>) -> Self
    where
        Self: Sized,
    {
        let this = self.with_tokio_executor(rt.clone());
        #[cfg(unix)]
        let this = this.with_tokio_reactor(rt);
        this
    }

    fn with_tokio_executor(self, rt: Arc<Runtime>) -> Self
    where
        Self: Sized;

    #[cfg(unix)]
    fn with_tokio_reactor(self, rt: Arc<Runtime>) -> Self
    where
        Self: Sized;
}

impl LapinTokioExt for ConnectionProperties {
    fn with_tokio_executor(self, rt: Arc<Runtime>) -> Self {
        self.with_executor(TokioExecutor(rt))
    }

    #[cfg(unix)]
    fn with_tokio_reactor(self, rt: Arc<Runtime>) -> Self {
        self.with_reactor(unix::TokioReactorBuilder(Arc::new(TokioExecutor(rt))))
    }
}

#[derive(Debug)]
struct TokioExecutor(Arc<Runtime>);

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
    use super::*;
    use lapin::{
        heartbeat::Heartbeat,
        reactor::{Reactor, ReactorBuilder, ReactorHandle, Slot},
        socket_state::{SocketEvent, SocketStateHandle},
        tcp::{TcpStream, TcpStreamWrapper},
    };
    use parking_lot::Mutex;
    use std::{collections::HashMap, fmt, sync::Arc};
    use tokio::{io::unix::AsyncFd, time::sleep};

    #[derive(Debug)]
    pub(crate) struct TokioReactorBuilder(pub(crate) Arc<dyn Executor>);

    #[derive(Debug)]
    struct TokioReactor(TokioReactorHandle);

    #[derive(Clone)]
    struct TokioReactorHandle {
        heartbeat: Heartbeat,
        executor: Arc<dyn Executor>,
        inner: Arc<Mutex<Inner>>,
    }

    #[derive(Default)]
    struct Inner {
        slot: Slot,
        slots: HashMap<Slot, (Arc<AsyncFd<TcpStreamWrapper>>, SocketStateHandle)>,
    }

    impl fmt::Debug for TokioReactorHandle {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TokioReactorHandle").finish()
        }
    }

    impl Inner {
        fn register(
            &mut self,
            socket: Arc<AsyncFd<TcpStreamWrapper>>,
            socket_state: SocketStateHandle,
        ) -> Result<usize> {
            let slot = self.slot;
            self.slot += 1;
            self.slots.insert(slot, (socket, socket_state));
            Ok(slot)
        }
    }

    impl ReactorBuilder for TokioReactorBuilder {
        fn build(&self, heartbeat: Heartbeat) -> Result<Box<dyn Reactor + Send>> {
            Ok(Box::new(TokioReactor(TokioReactorHandle {
                heartbeat,
                executor: self.0.clone(),
                inner: Arc::new(Mutex::new(Default::default())),
            })))
        }
    }

    impl Reactor for TokioReactor {
        fn register(
            &mut self,
            socket: &mut TcpStream,
            socket_state: SocketStateHandle,
        ) -> Result<Slot> {
            let socket = Arc::new(AsyncFd::new(unsafe { TcpStreamWrapper::new(socket) })?);
            let slot = self.0.inner.lock().register(socket, socket_state)?;
            self.0.poll_read(slot);
            self.0.poll_write(slot);
            Ok(slot)
        }

        fn handle(&self) -> Box<dyn ReactorHandle + Send> {
            Box::new(self.0.clone())
        }
    }

    impl ReactorHandle for TokioReactorHandle {
        fn start_heartbeat(&self) {
            self.executor
                .spawn(Box::pin(heartbeat(self.heartbeat.clone())))
                .expect("start_heartbeat");
        }

        fn poll_read(&self, slot: usize) {
            if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
                self.executor
                    .spawn(Box::pin(poll_read(socket.clone(), socket_state.clone())))
                    .expect("poll_read");
            }
        }

        fn poll_write(&self, slot: usize) {
            if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
                self.executor
                    .spawn(Box::pin(poll_write(socket.clone(), socket_state.clone())))
                    .expect("poll_write");
            }
        }
    }

    async fn heartbeat(heartbeat: Heartbeat) {
        while let Ok(Some(timeout)) = heartbeat.poll_timeout() {
            sleep(timeout).await;
        }
    }

    async fn poll_read(socket: Arc<AsyncFd<TcpStreamWrapper>>, socket_state: SocketStateHandle) {
        socket.readable().await.unwrap().clear_ready();
        socket_state.send(SocketEvent::Readable);
    }

    async fn poll_write(socket: Arc<AsyncFd<TcpStreamWrapper>>, socket_state: SocketStateHandle) {
        socket.writable().await.unwrap().clear_ready();
        socket_state.send(SocketEvent::Writable);
    }
}
