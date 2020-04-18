use crate::{
    channels::Channels,
    executor::{Executor, ExecutorExt},
    socket_state::SocketStateHandle,
    types::ShortUInt,
    Error, Promise, Result,
};
use crossbeam_channel::{Receiver, Sender};
use log::trace;
use std::sync::Arc;

pub(crate) struct InternalRPC {
    rpc: Receiver<InternalCommand>,
    executor: Arc<dyn Executor>,
    handle: InternalRPCHandle,
}

#[derive(Clone)]
pub(crate) struct InternalRPCHandle {
    sender: Sender<InternalCommand>,
    waker: SocketStateHandle,
}

impl InternalRPCHandle {
    pub(crate) fn close_connection(
        &self,
        reply_code: ShortUInt,
        reply_text: String,
        class_id: ShortUInt,
        method_id: ShortUInt,
    ) {
        self.send(InternalCommand::CloseConnection(
            reply_code, reply_text, class_id, method_id,
        ));
    }

    pub(crate) fn send_connection_close_ok(&self, error: Error) {
        self.send(InternalCommand::SendConnectionCloseOk(error));
    }

    pub(crate) fn remove_channel(&self, channel_id: u16, error: Error) {
        self.send(InternalCommand::RemoveChannel(channel_id, error));
    }

    pub(crate) fn set_connection_closing(&self) {
        self.send(InternalCommand::SetConnectionClosing);
    }

    pub(crate) fn set_connection_closed(&self, error: Error) {
        self.send(InternalCommand::SetConnectionClosed(error));
    }

    pub(crate) fn set_connection_error(&self, error: Error) {
        self.send(InternalCommand::SetConnectionError(error));
    }

    fn send(&self, command: InternalCommand) {
        trace!("Queuing internal RPC command: {:?}", command);
        self.sender.send(command).expect("internal RPC failed");
        self.waker.wake();
    }
}

#[derive(Debug)]
enum InternalCommand {
    CloseConnection(ShortUInt, String, ShortUInt, ShortUInt),
    SendConnectionCloseOk(Error),
    RemoveChannel(u16, Error),
    SetConnectionClosing,
    SetConnectionClosed(Error),
    SetConnectionError(Error),
}

impl InternalRPC {
    pub(crate) fn new(executor: Arc<dyn Executor>, waker: SocketStateHandle) -> Self {
        let (sender, rpc) = crossbeam_channel::unbounded();
        let handle = InternalRPCHandle { sender, waker };
        Self {
            rpc,
            executor,
            handle,
        }
    }

    pub(crate) fn handle(&self) -> InternalRPCHandle {
        self.handle.clone()
    }

    pub(crate) fn poll(&self, channels: &Channels) -> Result<()> {
        while let Ok(command) = self.rpc.try_recv() {
            self.run(command, channels)?;
        }
        Ok(())
    }

    fn run(&self, command: InternalCommand, channels: &Channels) -> Result<()> {
        use InternalCommand::*;

        trace!("Handling internal RPC command: {:?}", command);
        match command {
            CloseConnection(reply_code, reply_text, class_id, method_id) => channels
                .get(0)
                .map(|channel0| {
                    self.register_internal_promise(channel0.connection_close(
                        reply_code,
                        &reply_text,
                        class_id,
                        method_id,
                    ))
                })
                .unwrap_or(Ok(())),
            SendConnectionCloseOk(error) => channels
                .get(0)
                .map(|channel| self.register_internal_promise(channel.connection_close_ok(error)))
                .unwrap_or(Ok(())),
            RemoveChannel(channel_id, error) => channels.remove(channel_id, error),
            SetConnectionClosing => {
                channels.set_connection_closing();
                Ok(())
            }
            SetConnectionClosed(error) => channels.set_connection_closed(error),
            SetConnectionError(error) => channels.set_connection_error(error),
        }
    }

    fn register_internal_promise(&self, promise: Promise<()>) -> Result<()> {
        self.executor.spawn_internal(promise, self.handle())
    }
}
