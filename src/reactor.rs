use crate::{
    executor::Executor,
    heartbeat::Heartbeat,
    socket_state::{SocketEvent, SocketStateHandle},
    tcp::{TcpStream, TcpStreamWrapper},
    Result,
};
use async_io::{Async, Timer};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc};

pub type Slot = usize;

pub trait ReactorBuilder: fmt::Debug + Send + Sync {
    fn build(&self, heartbeat: Heartbeat, executor: Arc<dyn Executor>) -> Box<dyn Reactor + Send>;
}

pub trait Reactor: fmt::Debug + Send {
    fn register(&mut self, socket: &mut TcpStream, socket_state: SocketStateHandle)
        -> Result<Slot>;
    fn handle(&self) -> Box<dyn ReactorHandle + Send> {
        Box::new(DummyHandle)
    }
}

pub trait ReactorHandle {
    fn start_heartbeat(&self) {}
    fn poll_read(&self, _slot: Slot) {}
    fn poll_write(&self, _slot: Slot) {}
}

#[derive(Clone)]
struct DummyHandle;

impl ReactorHandle for DummyHandle {}

pub(crate) struct DefaultReactorBuilder;

impl ReactorBuilder for DefaultReactorBuilder {
    fn build(&self, heartbeat: Heartbeat, executor: Arc<dyn Executor>) -> Box<dyn Reactor + Send> {
        Box::new(DefaultReactor(DefaultReactorHandle {
            heartbeat,
            executor,
            inner: Arc::new(Mutex::new(Default::default())),
        }))
    }
}

#[derive(Debug)]
pub(crate) struct DefaultReactor(DefaultReactorHandle);

impl Reactor for DefaultReactor {
    fn register(
        &mut self,
        socket: &mut TcpStream,
        socket_state: SocketStateHandle,
    ) -> Result<Slot> {
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

#[derive(Clone)]
pub(crate) struct DefaultReactorHandle {
    heartbeat: Heartbeat,
    executor: Arc<dyn Executor>,
    inner: Arc<Mutex<Inner>>,
}

impl fmt::Debug for DefaultReactorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultReactorHandle").finish()
    }
}

impl ReactorHandle for DefaultReactorHandle {
    fn start_heartbeat(&self) {
        self.executor
            .spawn(Box::pin(heartbeat(self.heartbeat.clone())));
    }

    fn poll_read(&self, slot: usize) {
        if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
            self.executor
                .spawn(Box::pin(poll_read(socket.clone(), socket_state.clone())));
        }
    }

    fn poll_write(&self, slot: usize) {
        if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
            self.executor
                .spawn(Box::pin(poll_write(socket.clone(), socket_state.clone())));
        }
    }
}

#[derive(Default)]
struct Inner {
    slot: Slot,
    slots: HashMap<usize, (Arc<Async<TcpStreamWrapper>>, SocketStateHandle)>,
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

async fn heartbeat(heartbeat: Heartbeat) {
    while let Some(timeout) = heartbeat.poll_timeout() {
        Timer::after(timeout).await;
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

impl fmt::Debug for DefaultReactorBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultReactorBuilder").finish()
    }
}
