use crate::{executor::Executor, heartbeat::Heartbeat, Result, TcpStream};
use async_io::{Async, Timer};
use futures_lite::io::{AsyncRead, AsyncWrite};
use std::{fmt, sync::Arc};

pub trait AsyncRW: AsyncRead + AsyncWrite {}
impl<IO: AsyncRead + AsyncWrite> AsyncRW for IO {}

pub trait ReactorBuilder: fmt::Debug + Send + Sync {
    fn build(&self, heartbeat: Heartbeat, executor: Arc<dyn Executor>) -> Box<dyn Reactor + Send>;
}

pub trait Reactor: fmt::Debug + Send {
    fn register(&mut self, socket: TcpStream) -> Result<Box<dyn AsyncRW + Send>>;
    fn handle(&self) -> Box<dyn ReactorHandle + Send> {
        Box::new(DummyHandle)
    }
}

pub trait ReactorHandle {
    fn start_heartbeat(&self) {}
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
        }))
    }
}

impl fmt::Debug for DefaultReactorBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultReactorBuilder").finish()
    }
}

#[derive(Debug)]
pub(crate) struct DefaultReactor(DefaultReactorHandle);

impl Reactor for DefaultReactor {
    fn register(&mut self, socket: TcpStream) -> Result<Box<dyn AsyncRW + Send>> {
        Ok(Box::new(Async::new(socket)?))
    }

    fn handle(&self) -> Box<dyn ReactorHandle + Send> {
        Box::new(self.0.clone())
    }
}

#[derive(Clone)]
pub(crate) struct DefaultReactorHandle {
    heartbeat: Heartbeat,
    executor: Arc<dyn Executor>,
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
}

async fn heartbeat(heartbeat: Heartbeat) {
    while let Some(timeout) = heartbeat.poll_timeout() {
        Timer::after(timeout).await;
    }
}
