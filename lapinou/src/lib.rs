use lapin::{
    executor::Executor,
    heartbeat::Heartbeat,
    reactor::{Reactor, ReactorBuilder, ReactorHandle, Slot},
    socket_state::{SocketEvent, SocketStateHandle},
    tcp::{TcpStream, TcpStreamWrapper},
    ConnectionProperties, Result,
};
use parking_lot::Mutex;
use smol::{Async, Task, Timer};
use std::{collections::HashMap, fmt, future::Future, pin::Pin, sync::Arc};

// ConnectionProperties extension

pub trait LapinSmolExt {
    fn with_smol(self) -> Self
    where
        Self: Sized;

    fn with_smol_executor(self) -> Self
    where
        Self: Sized;

    fn with_smol_reactor(self) -> Self
    where
        Self: Sized;
}

impl LapinSmolExt for ConnectionProperties {
    fn with_smol(self) -> Self {
        self.with_smol_executor().with_smol_reactor()
    }

    fn with_smol_executor(self) -> Self {
        self.with_executor(SmolExecutor)
    }

    fn with_smol_reactor(self) -> Self {
        self.with_reactor(SmolReactorBuilder)
    }
}

// Executor

#[derive(Debug)]
struct SmolExecutor;

impl Executor for SmolExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        Task::spawn(f).detach();
        Ok(())
    }
}

// Reactor

#[derive(Debug)]
struct SmolReactorBuilder;

#[derive(Debug)]
struct SmolReactor(SmolReactorHandle);

#[derive(Clone)]
struct SmolReactorHandle {
    heartbeat: Heartbeat,
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    slot: Slot,
    slots: HashMap<usize, (Arc<Async<TcpStreamWrapper>>, SocketStateHandle)>,
}

impl fmt::Debug for SmolReactorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmolReactorHandle").finish()
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

impl ReactorBuilder for SmolReactorBuilder {
    fn build(&self, heartbeat: Heartbeat) -> Result<Box<dyn Reactor + Send>> {
        Ok(Box::new(SmolReactor(SmolReactorHandle {
            heartbeat,
            inner: Arc::new(Mutex::new(Inner::default())),
        })))
    }
}

impl Reactor for SmolReactor {
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

impl ReactorHandle for SmolReactorHandle {
    fn start_heartbeat(&self) {
        Task::spawn(heartbeat(self.heartbeat.clone())).detach();
    }

    fn poll_read(&self, slot: usize) {
        if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
            Task::spawn(poll_read(socket.clone(), socket_state.clone()))
                .unwrap()
                .detach();
        }
    }

    fn poll_write(&self, slot: usize) {
        if let Some((socket, socket_state)) = self.inner.lock().slots.get(&slot) {
            Task::spawn(poll_write(socket.clone(), socket_state.clone()))
                .unwrap()
                .detach();
        }
    }
}

async fn heartbeat(heartbeat: Heartbeat) {
    while let Ok(Some(timeout)) = heartbeat.poll_timeout() {
        Timer::after(timeout).await;
    }
}

async fn poll_read(
    socket: Arc<Async<TcpStreamWrapper>>,
    socket_state: SocketStateHandle,
) -> Result<()> {
    socket.read_with(|stream| stream.is_readable()).await?;
    socket_state.send(SocketEvent::Readable);
    Ok(())
}

async fn poll_write(
    socket: Arc<Async<TcpStreamWrapper>>,
    socket_state: SocketStateHandle,
) -> Result<()> {
    socket.write_with(|stream| stream.is_writable()).await?;
    socket_state.send(SocketEvent::Writable);
    Ok(())
}
