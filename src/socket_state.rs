use crate::{
    reactor::{ReactorHandle, Slot},
    Result,
};
use crossbeam_channel::{Receiver, Sender};
use log::trace;

pub(crate) struct SocketState {
    readable: bool,
    writable: bool,
    error: bool,
    closed: bool,
    events: Receiver<SocketEvent>,
    handle: SocketStateHandle,
}

impl Default for SocketState {
    fn default() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self {
            readable: true,
            writable: true,
            error: false,
            closed: false,
            events: receiver,
            handle: SocketStateHandle { sender },
        }
    }
}

#[derive(Clone)]
pub struct SocketStateHandle {
    sender: Sender<SocketEvent>,
}

#[derive(Debug)]
pub enum SocketEvent {
    Readable,
    Writable,
    Error,
    Closed,
    Wake,
}

impl SocketState {
    pub(crate) fn readable(&mut self) -> bool {
        self.readable
    }

    pub(crate) fn writable(&mut self) -> bool {
        self.writable
    }

    pub(crate) fn error(&mut self) -> bool {
        self.error
    }

    pub(crate) fn closed(&mut self) -> bool {
        self.closed
    }

    pub(crate) fn wait(&mut self) {
        self.handle_event(self.events.recv().expect("waiting for socket event failed"))
    }

    pub(crate) fn handle_read_result(
        &mut self,
        result: Result<()>,
        reactor: &dyn ReactorHandle,
        slot: Slot,
    ) -> Result<()> {
        self.readable = Self::handle_io_result(result)?;
        if !self.readable {
            reactor.poll_read(slot);
        }
        Ok(())
    }

    pub(crate) fn handle_write_result(
        &mut self,
        result: Result<()>,
        reactor: &dyn ReactorHandle,
        slot: Slot,
    ) -> Result<()> {
        self.writable = Self::handle_io_result(result)?;
        if !self.writable {
            reactor.poll_write(slot);
        }
        Ok(())
    }

    pub(crate) fn handle(&self) -> SocketStateHandle {
        self.handle.clone()
    }

    fn handle_io_result(result: Result<()>) -> Result<bool> {
        if let Err(err) = result {
            if err.wouldblock() {
                Ok(false)
            } else {
                Err(err)
            }
        } else {
            Ok(true)
        }
    }

    pub(crate) fn poll_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: SocketEvent) {
        trace!("Got event for socket: {:?}", event);
        match event {
            SocketEvent::Readable => self.readable = true,
            SocketEvent::Writable => self.writable = true,
            SocketEvent::Error => self.error = true,
            SocketEvent::Closed => self.closed = true,
            SocketEvent::Wake => {}
        }
    }
}

impl SocketStateHandle {
    pub fn send(&self, event: SocketEvent) {
        self.sender
            .send(event)
            .expect("forwarding soket event failed")
    }

    pub fn wake(&self) {
        self.send(SocketEvent::Wake);
    }
}
