use crate::{
    heartbeat::Heartbeat,
    socket_state::{SocketEvent, SocketStateHandle},
    thread::ThreadHandle,
    Result,
};
use log::trace;
use mio::{event::Source, Events, Interest, Poll, Token};
use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::Builder as ThreadBuilder,
};

pub trait ReactorBuilder: fmt::Debug + Send + Sync {
    fn build(&self, poll: Poll, heartbeat: Heartbeat) -> Box<dyn Reactor + Send>;
}

pub trait Reactor: fmt::Debug + Send {
    fn handle(&self) -> Box<dyn ReactorHandle + Send>;
    fn register(
        &mut self,
        token: Token,
        socket_state: SocketStateHandle,
        socket: &mut dyn Source,
        reregister: bool,
    ) -> Result<()>;
    fn start(self: Box<Self>) -> Result<ThreadHandle>;
}

pub trait ReactorHandle {
    fn shutdown(&self);
}

pub(crate) struct DefaultReactorBuilder;

impl ReactorBuilder for DefaultReactorBuilder {
    fn build(&self, poll: Poll, heartbeat: Heartbeat) -> Box<dyn Reactor + Send> {
        Box::new(DefaultReactor::new(poll, heartbeat))
    }
}

pub(crate) struct DefaultReactor {
    poll: Poll,
    heartbeat: Heartbeat,
    slots: HashMap<Token, SocketStateHandle>,
    handle: DefaultReactorHandle,
}

#[derive(Clone)]
pub(crate) struct DefaultReactorHandle(Arc<AtomicBool>);

impl Reactor for DefaultReactor {
    fn handle(&self) -> Box<dyn ReactorHandle + Send> {
        Box::new(self.handle.clone())
    }

    fn register(
        &mut self,
        token: Token,
        socket_state: SocketStateHandle,
        socket: &mut dyn Source,
        reregister: bool,
    ) -> Result<()> {
        if reregister {
            self.poll.registry().reregister(
                socket,
                token,
                Interest::READABLE | Interest::WRITABLE,
            )?;
        } else {
            self.poll.registry().reregister(
                socket,
                token,
                Interest::READABLE | Interest::WRITABLE,
            )?;
        }
        self.slots.insert(token, socket_state);
        Ok(())
    }

    fn start(mut self: Box<Self>) -> Result<ThreadHandle> {
        Ok(ThreadHandle::new(
            ThreadBuilder::new()
                .name("lapin-reactor".into())
                .spawn(move || {
                    let mut events = Events::with_capacity(16);
                    while self.should_run() {
                        self.run(&mut events)?;
                    }
                    Ok(())
                })?,
        ))
    }
}

impl DefaultReactor {
    fn new(poll: Poll, heartbeat: Heartbeat) -> Self {
        Self {
            poll,
            heartbeat,
            slots: HashMap::default(),
            handle: DefaultReactorHandle(Arc::new(AtomicBool::new(true))),
        }
    }

    fn should_run(&self) -> bool {
        self.handle.0.load(Ordering::SeqCst)
    }

    fn run(&mut self, events: &mut Events) -> Result<()> {
        trace!("reactor poll");
        self.poll.poll(events, self.heartbeat.poll_timeout()?)?;
        trace!("reactor poll done");
        for event in events.iter() {
            if let Some(socket) = self.slots.get(&event.token()) {
                if event.is_error() {
                    socket.send(SocketEvent::Error);
                }
                if event.is_read_closed() || event.is_write_closed() {
                    socket.send(SocketEvent::Closed);
                }
                if event.is_readable() {
                    socket.send(SocketEvent::Readable);
                }
                if event.is_writable() {
                    socket.send(SocketEvent::Writable);
                }
            }
        }
        Ok(())
    }
}

impl ReactorHandle for DefaultReactorHandle {
    fn shutdown(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl fmt::Debug for DefaultReactorBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultReactorBuilder").finish()
    }
}

impl fmt::Debug for DefaultReactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultReactor").finish()
    }
}
