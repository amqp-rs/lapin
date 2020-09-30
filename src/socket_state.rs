use crate::{
    reactor::{ReactorHandle, Slot},
    Result,
};
use flume::{Receiver, Sender};
use tracing::trace;

pub(crate) struct SocketState {
    readable: bool,
    writable: bool,
    events: Receiver<SocketEvent>,
    handle: SocketStateHandle,
}

impl Default for SocketState {
    fn default() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self {
            readable: true,
            writable: true,
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
    Wake,
}

impl SocketState {
    pub(crate) fn readable(&mut self) -> bool {
        self.readable
    }

    pub(crate) fn writable(&mut self) -> bool {
        self.writable
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
        self.readable = self.handle_io_result(result, self.readable)?;
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
        self.writable = self.handle_io_result(result, self.writable)?;
        if !self.writable {
            reactor.poll_write(slot);
        }
        Ok(())
    }

    pub(crate) fn handle(&self) -> SocketStateHandle {
        self.handle.clone()
    }

    fn handle_io_result(&self, result: Result<()>, current: bool) -> Result<bool> {
        if let Err(err) = result {
            if err.wouldblock() {
                Ok(false)
            } else if err.interrupted() {
                self.handle.wake();
                Ok(current)
            } else {
                Err(err)
            }
        } else {
            Ok(current)
        }
    }

    pub(crate) fn poll_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.handle_event(event);
        }
    }

    fn handle_event(&mut self, event: SocketEvent) {
        trace!(?event, "Got event for socket");
        match event {
            SocketEvent::Readable => self.readable = true,
            SocketEvent::Writable => self.writable = true,
            SocketEvent::Wake => {}
        }
    }
}

impl SocketStateHandle {
    pub fn send(&self, event: SocketEvent) {
        let _ = self.sender.send(event);
    }

    pub fn wake(&self) {
        self.send(SocketEvent::Wake);
    }
}
