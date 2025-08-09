use crate::{
    Error, ErrorKind, PromiseResolver, Result,
    channels::Channels,
    consumer_status::ConsumerStatus,
    error_holder::ErrorHolder,
    killswitch::KillSwitch,
    options::{BasicAckOptions, BasicCancelOptions, BasicNackOptions, BasicRejectOptions},
    reactor::FullReactor,
    socket_state::SocketStateHandle,
    types::{ChannelId, DeliveryTag, Identifier, ReplyCode},
};
use executor_trait::FullExecutor;
use flume::{Receiver, Sender};
use reactor_trait::{AsyncIOHandle, IOHandle};
use std::{collections::HashMap, fmt, future::Future, sync::Arc};
use tracing::trace;

pub(crate) struct InternalRPC {
    rpc: Receiver<Option<InternalCommand>>,
    handle: InternalRPCHandle,
    channels_status: HashMap<ChannelId, KillSwitch>,
    reactor: Arc<dyn FullReactor + Send + Sync>,
}

#[derive(Clone)]
pub(crate) struct InternalRPCHandle {
    sender: Sender<Option<InternalCommand>>,
    waker: SocketStateHandle,
    executor: Arc<dyn FullExecutor + Send + Sync>,
}

impl InternalRPCHandle {
    pub(crate) fn basic_ack(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicAckOptions,
        resolver: PromiseResolver<()>,
        error: Option<ErrorHolder>,
    ) {
        self.send(InternalCommand::BasicAck(
            channel_id,
            delivery_tag,
            options,
            resolver,
            error,
        ));
    }

    pub(crate) fn basic_nack(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicNackOptions,
        resolver: PromiseResolver<()>,
        error: Option<ErrorHolder>,
    ) {
        self.send(InternalCommand::BasicNack(
            channel_id,
            delivery_tag,
            options,
            resolver,
            error,
        ));
    }

    pub(crate) fn basic_reject(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicRejectOptions,
        resolver: PromiseResolver<()>,
        error: Option<ErrorHolder>,
    ) {
        self.send(InternalCommand::BasicReject(
            channel_id,
            delivery_tag,
            options,
            resolver,
            error,
        ));
    }

    pub(crate) fn cancel_consumer(
        &self,
        channel_id: ChannelId,
        consumer_tag: String,
        consumer_status: ConsumerStatus,
    ) {
        self.send(InternalCommand::CancelConsumer(
            channel_id,
            consumer_tag,
            consumer_status,
        ));
    }

    pub(crate) fn close_channel(
        &self,
        channel_id: ChannelId,
        reply_code: ReplyCode,
        reply_text: String,
    ) {
        self.send(InternalCommand::CloseChannel(
            channel_id, reply_code, reply_text,
        ));
    }

    pub(crate) fn close_connection(
        &self,
        reply_code: ReplyCode,
        reply_text: String,
        class_id: Identifier,
        method_id: Identifier,
    ) {
        self.set_connection_closing();
        self.send(InternalCommand::CloseConnection(
            reply_code, reply_text, class_id, method_id,
        ));
    }

    pub(crate) fn finish_connection_shutdown(&self) {
        self.send(InternalCommand::FinishConnectionShutdown);
    }

    pub(crate) fn init_connection_recovery(&self, error: Error) {
        self.send(InternalCommand::InitConnectionRecovery(error));
    }

    pub(crate) fn init_connection_shutdown(&self, error: Error) {
        self.send(InternalCommand::InitConnectionShutdown(error));
    }

    pub(crate) fn reactor_register(
        &self,
        handle: IOHandle,
        resolver: PromiseResolver<Box<dyn AsyncIOHandle + Send>>,
    ) {
        self.send(InternalCommand::ReactorRegister(handle, resolver));
    }

    pub(crate) fn remove_channel(&self, channel_id: ChannelId, error: Error) {
        self.send(InternalCommand::RemoveChannel(channel_id, error));
    }

    pub(crate) fn send_connection_close_ok(&self, error: Error) {
        self.send(InternalCommand::SendConnectionCloseOk(error));
    }

    pub(crate) fn set_channel_status(&self, channel_id: ChannelId, killswitch: KillSwitch) {
        self.send(InternalCommand::SetChannelStatus(channel_id, killswitch));
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

    pub(crate) fn start_channels_recovery(&self) {
        self.send(InternalCommand::StartChannelsRecovery);
    }

    pub(crate) fn stop(&self) {
        trace!("Stopping internal RPC command");
        let _ = self.sender.send(None);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sender.is_empty()
    }

    fn send(&self, command: InternalCommand) {
        trace!(?command, "Queuing internal RPC command");
        // The only scenario where this can fail if this is the IoLoop already exited
        let _ = self.sender.send(Some(command));
        self.waker.wake();
    }

    pub(crate) fn register_internal_future(
        &self,
        f: impl Future<Output = Result<()>> + Send + 'static,
    ) {
        let handle = self.clone();
        self.executor.spawn(Box::pin(async move {
            if let Err(err) = f.await {
                handle.set_connection_error(err);
            }
        }));
    }

    fn register_internal_future_with_resolver(
        &self,
        f: impl Future<Output = Result<()>> + Send + 'static,
        resolver: PromiseResolver<()>,
    ) {
        self.executor.spawn(Box::pin(async move {
            let res = f.await;
            resolver.complete(res);
        }));
    }
}

impl fmt::Debug for InternalRPCHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InternalRPCHandle").finish()
    }
}

#[derive(Debug)]
enum InternalCommand {
    BasicAck(
        ChannelId,
        DeliveryTag,
        BasicAckOptions,
        PromiseResolver<()>,
        Option<ErrorHolder>,
    ),
    BasicNack(
        ChannelId,
        DeliveryTag,
        BasicNackOptions,
        PromiseResolver<()>,
        Option<ErrorHolder>,
    ),
    BasicReject(
        ChannelId,
        DeliveryTag,
        BasicRejectOptions,
        PromiseResolver<()>,
        Option<ErrorHolder>,
    ),
    CancelConsumer(ChannelId, String, ConsumerStatus),
    CloseChannel(ChannelId, ReplyCode, String),
    CloseConnection(ReplyCode, String, Identifier, Identifier),
    FinishConnectionShutdown,
    InitConnectionRecovery(Error),
    InitConnectionShutdown(Error),
    ReactorRegister(IOHandle, PromiseResolver<Box<dyn AsyncIOHandle + Send>>),
    RemoveChannel(ChannelId, Error),
    SendConnectionCloseOk(Error),
    SetChannelStatus(ChannelId, KillSwitch),
    SetConnectionClosing,
    SetConnectionClosed(Error),
    SetConnectionError(Error),
    StartChannelsRecovery,
}

impl InternalRPC {
    pub(crate) fn new(
        executor: Arc<dyn FullExecutor + Send + Sync>,
        reactor: Arc<dyn FullReactor + Send + Sync>,
        waker: SocketStateHandle,
    ) -> Self {
        let (sender, rpc) = flume::unbounded();
        let handle = InternalRPCHandle {
            sender,
            waker,
            executor,
        };
        Self {
            rpc,
            handle,
            channels_status: Default::default(),
            reactor,
        }
    }

    pub(crate) fn handle(&self) -> InternalRPCHandle {
        self.handle.clone()
    }

    fn channel_ok(&self, chan: ChannelId) -> bool {
        self.channels_status
            .get(&chan)
            .is_some_and(|killswitch| !killswitch.killed())
    }

    pub(crate) fn start(self, channels: Channels) {
        self.handle().executor.spawn(Box::pin(self.run(channels)));
    }

    async fn run(mut self, channels: Channels) {
        use InternalCommand::*;

        let rpc = self.rpc.clone();
        let handle = self.handle();
        let get_channel = |id| {
            channels
                .get(id)
                .ok_or::<Error>(ErrorKind::InvalidChannel(id).into())
        };

        while let Ok(Some(command)) = rpc.recv_async().await {
            trace!(?command, "Handling internal RPC command");
            match command {
                BasicAck(channel_id, delivery_tag, options, resolver, error) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    handle.register_internal_future_with_resolver(
                        async move {
                            if let Some(error) = error {
                                error.check()?;
                            }
                            channel?.basic_ack(delivery_tag, options).await
                        },
                        resolver,
                    )
                }
                BasicNack(channel_id, delivery_tag, options, resolver, error) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    handle.register_internal_future_with_resolver(
                        async move {
                            if let Some(error) = error {
                                error.check()?;
                            }
                            channel?.basic_nack(delivery_tag, options).await
                        },
                        resolver,
                    )
                }
                BasicReject(channel_id, delivery_tag, options, resolver, error) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    handle.register_internal_future_with_resolver(
                        async move {
                            if let Some(error) = error {
                                error.check()?;
                            }
                            channel?.basic_reject(delivery_tag, options).await
                        },
                        resolver,
                    )
                }
                CancelConsumer(channel_id, consumer_tag, consumer_status) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    handle.register_internal_future(async move {
                        let channel = channel?;
                        if channel.status().connected() && consumer_status.state().is_active() {
                            channel
                                .basic_cancel(&consumer_tag, BasicCancelOptions::default())
                                .await
                        } else {
                            Ok(())
                        }
                    })
                }
                CloseChannel(channel_id, reply_code, reply_text) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    handle.register_internal_future(async move {
                        channel?.close(reply_code, &reply_text).await
                    })
                }
                CloseConnection(reply_code, reply_text, class_id, method_id) => {
                    let channel = channels.channel0();
                    handle.register_internal_future(async move {
                        channel
                            .connection_close(reply_code, &reply_text, class_id, method_id)
                            .await
                    })
                }
                FinishConnectionShutdown => channels.finish_connection_shutdown(),
                InitConnectionRecovery(error) => {
                    channels.init_connection_recovery(error);
                }
                InitConnectionShutdown(error) => channels.init_connection_shutdown(error),
                ReactorRegister(handle, resolver) => {
                    resolver.complete(self.reactor.register(handle).map_err(Error::from));
                }
                RemoveChannel(channel_id, error) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channels = channels.clone();
                    handle
                        .register_internal_future(async move { channels.remove(channel_id, error) })
                }
                SendConnectionCloseOk(error) => {
                    let channel = channels.channel0();
                    handle.register_internal_future(async move {
                        channel.connection_close_ok(error).await
                    })
                }
                SetChannelStatus(channel_id, killswitch) => {
                    self.channels_status.insert(channel_id, killswitch);
                }
                SetConnectionClosing => channels.set_connection_closing(),
                SetConnectionClosed(error) => channels.set_connection_closed(error),
                SetConnectionError(error) => channels.set_connection_error(error),
                StartChannelsRecovery => {
                    let channels = channels.clone();
                    handle.register_internal_future(async move { channels.start_recovery().await })
                }
            }
            handle.waker.wake();
        }
        trace!("InternalRPC stopped");
    }
}
