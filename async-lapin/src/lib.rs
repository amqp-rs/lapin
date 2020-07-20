use async_io::{Async, Timer};
use lapin::{
    heartbeat::Heartbeat,
    reactor::{Reactor, ReactorBuilder, ReactorHandle, Slot},
    socket_state::{SocketEvent, SocketStateHandle},
    tcp::{TcpStream, TcpStreamWrapper},
    ConnectionProperties, Result,
};
use simple_mutex::Mutex;
use std::{collections::HashMap, fmt, future::Future, pin::Pin, sync::Arc};

// ConnectionProperties extension

pub trait LapinAsyncIoExt {
    fn with_async_io(
        self,
        spawner: impl Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + Send + Sync + 'static,
    ) -> Self
    where
        Self: Sized,
    {
        self.with_async_io_reactor(spawner)
    }

    fn with_async_io_reactor(
        self,
        spawner: impl Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + Send + Sync + 'static,
    ) -> Self
    where
        Self: Sized;
}

impl LapinAsyncIoExt for ConnectionProperties {
    fn with_async_io_reactor(
        self,
        spawner: impl Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + Send + Sync + 'static,
    ) -> Self {
        self.with_reactor(AsyncIoReactorBuilder(Arc::new(spawner)))
    }
}

// Reactor

struct AsyncIoReactorBuilder(Arc<dyn Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + Send + Sync>);

impl fmt::Debug for AsyncIoReactorBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncIoReactorBuilder").finish()
    }
}

#[derive(Debug)]
struct AsyncIoReactor(AsyncIoReactorHandle);

#[derive(Clone)]
struct AsyncIoReactorHandle {
    heartbeat: Heartbeat,
    spawner: Arc<dyn Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + Send + Sync>,
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    slot: Slot,
    slots: HashMap<usize, (Arc<Async<TcpStreamWrapper>>, SocketStateHandle)>,
}

impl fmt::Debug for AsyncIoReactorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncIoReactorHandle").finish()
    }
}

impl Inner {
    fn register(
        &mut self,
        socket: Arc<Async<TcpStreamWrapper>>,
        socket_state: SocketStateHandle,
    ) -> Result<usize> {
        let slot = self.slot;
        self.slot += 1;
        self.slots.insert(slot, (socket, socket_state));
        Ok(slot)
    }
}

impl ReactorBuilder for AsyncIoReactorBuilder {
    fn build(&self, heartbeat: Heartbeat) -> Result<Box<dyn Reactor + Send>> {
        Ok(Box::new(AsyncIoReactor(AsyncIoReactorHandle {
            heartbeat,
            spawner: self.0.clone(),
            inner: Arc::new(Mutex::new(Default::default())),
        })))
    }
}

impl Reactor for AsyncIoReactor {
    fn register(
        &mut self,
        socket: &mut TcpStream,
        socket_state: SocketStateHandle,
    ) -> Result<usize> {
        let socket = Arc::new(Async::new(unsafe { TcpStreamWrapper::new(socket) })?);
        let slot = self.0.inner.lock().register(socket, socket_state)?;
        self.0.poll_read(slot);
        self.0.poll_write(slot);
        Ok(slot)
    }

    fn handle(&self) -> Box<dyn ReactorHandle + Send> {
        Box::new(self.0.clone())
    }
}

impl ReactorHandle for AsyncIoReactorHandle {
    fn start_heartbeat(&self) {
        (self.spawner)(Box::pin(heartbeat(self.heartbeat.clone())));
    }

    fn poll_read(&self, slot: usize) {
        if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
            (self.spawner)(Box::pin(poll_read(socket.clone(), socket_state.clone())));
        }
    }

    fn poll_write(&self, slot: usize) {
        if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
            (self.spawner)(Box::pin(poll_write(socket.clone(), socket_state.clone())));
        }
    }
}

async fn heartbeat(heartbeat: Heartbeat) {
    while let Ok(Some(timeout)) = heartbeat.poll_timeout() {
        Timer::new(timeout).await;
    }
}

async fn poll_read(socket: Arc<Async<TcpStreamWrapper>>, socket_state: SocketStateHandle) {
    socket.readable().await.unwrap();
    socket_state.send(SocketEvent::Readable);
}

async fn poll_write(socket: Arc<Async<TcpStreamWrapper>>, socket_state: SocketStateHandle) {
    socket.writable().await.unwrap();
    socket_state.send(SocketEvent::Writable);
}
