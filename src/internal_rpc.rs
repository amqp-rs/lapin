use crate::{
    channels::Channels, promises::Promises, types::ShortUInt, waker::Waker, Error, Result,
};
use crossbeam_channel::{Receiver, Sender};
use log::trace;

pub(crate) struct InternalRPC {
    rpc: Receiver<InternalCommand>,
    internal_promises: Promises<()>,
    handle: InternalRPCHandle,
}

#[derive(Clone)]
pub(crate) struct InternalRPCHandle {
    sender: Sender<InternalCommand>,
    waker: Waker,
}

impl InternalRPCHandle {
    pub(crate) fn close_connection(
        &self,
        reply_code: ShortUInt,
        reply_text: String,
        class_id: ShortUInt,
        method_id: ShortUInt,
    ) -> Result<()> {
        self.send(InternalCommand::CloseConnection(
            reply_code, reply_text, class_id, method_id,
        ))
    }

    pub(crate) fn send_connection_close_ok(&self, error: Error) -> Result<()> {
        self.send(InternalCommand::SendConnectionCloseOk(error))
    }

    pub(crate) fn remove_channel(&self, channel_id: u16, error: Error) -> Result<()> {
        self.send(InternalCommand::RemoveChannel(channel_id, error))
    }

    pub(crate) fn set_connection_closing(&self) -> Result<()> {
        self.send(InternalCommand::SetConnectionClosing)
    }

    pub(crate) fn set_connection_closed(&self, error: Error) -> Result<()> {
        self.send(InternalCommand::SetConnectionClosed(error))
    }

    pub(crate) fn set_connection_error(&self, error: Error) -> Result<()> {
        self.send(InternalCommand::SetConnectionError(error))
    }

    fn send(&self, command: InternalCommand) -> Result<()> {
        trace!("Queuing internal RPC command: {:?}", command);
        self.sender.send(command).expect("internal RPC failed");
        self.waker.wake()
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
    pub(crate) fn new(waker: Waker, internal_promises: Promises<()>) -> Self {
        let (sender, rpc) = crossbeam_channel::unbounded();
        let handle = InternalRPCHandle { sender, waker };
        Self {
            rpc,
            internal_promises,
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
        self.poll_internal_promises(channels)
    }

    fn run(&self, command: InternalCommand, channels: &Channels) -> Result<()> {
        use InternalCommand::*;

        trace!("Handling internal RPC command: {:?}", command);
        match command {
            CloseConnection(reply_code, reply_text, class_id, method_id) => channels
                .get(0)
                .and_then(|channel0| {
                    self.internal_promises.register(channel0.connection_close(
                        reply_code,
                        &reply_text,
                        class_id,
                        method_id,
                    ))
                })
                .unwrap_or(Ok(())),
            SendConnectionCloseOk(error) => channels
                .get(0)
                .and_then(|channel| {
                    self.internal_promises
                        .register(channel.connection_close_ok(error))
                })
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

    fn poll_internal_promises(&self, channels: &Channels) -> Result<()> {
        if let Some(results) = self.internal_promises.try_wait() {
            for res in results {
                if let Err(err) = res {
                    channels.set_connection_error(err)?;
                }
            }
        }
        Ok(())
    }
}
