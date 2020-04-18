use crate::{
    heartbeat::Heartbeat,
    socket_state::{SocketEvent, SocketStateHandle},
    Result,
};
use log::trace;
use mio::{event::Source, Events, Interest, Poll, Token};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
    thread::{Builder as ThreadBuilder, JoinHandle},
};

pub(crate) struct Reactor {
    poll: Poll,
    heartbeat: Heartbeat,
    slots: HashMap<Token, SocketStateHandle>,
    handle: ReactorHandle,
}

#[derive(Clone)]
pub(crate) struct ReactorHandle(Arc<AtomicBool>);

impl Reactor {
    pub(crate) fn new(poll: Poll, heartbeat: Heartbeat) -> Self {
        Self {
            poll,
            heartbeat,
            slots: HashMap::default(),
            handle: ReactorHandle(Arc::new(AtomicBool::new(true))),
        }
    }

    pub(crate) fn handle(&self) -> ReactorHandle {
        self.handle.clone()
    }

    pub(crate) fn register(
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

    pub(crate) fn start(mut self) -> Result<JoinHandle<Result<()>>> {
        Ok(ThreadBuilder::new()
            .name("lapin-reactor".into())
            .spawn(move || {
                let mut events = Events::with_capacity(16);
                while self.should_run() {
                    self.run(&mut events)?;
                }
                Ok(())
            })?)
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

impl ReactorHandle {
    pub(crate) fn shutdown(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
