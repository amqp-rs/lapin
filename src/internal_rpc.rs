use crate::{
    Error, ErrorKind, PromiseResolver, Result,
    channels::Channels,
    consumer_status::ConsumerStatus,
    error_holder::ErrorHolder,
    heartbeat::Heartbeat,
    killswitch::KillSwitch,
    options::{BasicAckOptions, BasicCancelOptions, BasicNackOptions, BasicRejectOptions},
    socket_state::SocketStateHandle,
    types::{ChannelId, DeliveryTag, Identifier, ReplyCode},
};
use async_rs::{Runtime, traits::*};
use flume::{Receiver, Sender};
use std::{collections::HashMap, fmt, future::Future, pin::Pin, time::Duration};
use tracing::trace;

pub(crate) struct InternalRPC<RK: RuntimeKit + Clone + Send + 'static> {
    rpc: Receiver<Option<InternalCommand>>,
    handle: InternalRPCHandle,
    channels_status: HashMap<ChannelId, KillSwitch>,
    heartbeat: Heartbeat<RK>,
    runtime: Runtime<RK>,
}

#[derive(Clone)]
pub(crate) struct InternalRPCHandle {
    sender: Sender<Option<InternalCommand>>,
    waker: SocketStateHandle,
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

    pub(crate) fn cancel_heartbeat(&self) {
        self.send(InternalCommand::CancelHeartbeat);
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

    pub(crate) fn remove_channel(&self, channel_id: ChannelId, error: Error) {
        self.send(InternalCommand::RemoveChannel(channel_id, error));
    }

    pub(crate) fn reset_heartbeat(&self) {
        self.send(InternalCommand::ResetHeartbeat);
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

    pub(crate) fn spawn(&self, f: impl Future<Output = Result<()>> + Send + 'static) {
        self.send(InternalCommand::Spawn(Box::pin(f)));
    }

    pub(crate) fn spawn_infallible(&self, f: impl Future<Output = ()> + Send + 'static) {
        self.send(InternalCommand::Spawn(Box::pin(async move {
            f.await;
            Ok(())
        })));
    }

    pub(crate) fn start_channels_recovery(&self) {
        self.send(InternalCommand::StartChannelsRecovery);
    }

    pub(crate) fn start_heartbeat(&self, heartbeat: Duration) {
        self.send(InternalCommand::StartHeartbeat(heartbeat));
    }

    pub(crate) fn stop(&self) {
        trace!("Stopping internal RPC command");
        let _ = self.sender.send(None);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sender.is_empty()
    }

    fn send(&self, command: InternalCommand) {
        trace!("Queuing internal RPC command"); // FIXME: restore ?command (future is not debug for Spawn)
        // The only scenario where this can fail if this is the IoLoop already exited
        let _ = self.sender.send(Some(command));
        self.waker.wake();
    }
}

impl fmt::Debug for InternalRPCHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InternalRPCHandle").finish()
    }
}

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
    CancelHeartbeat,
    CloseChannel(ChannelId, ReplyCode, String),
    CloseConnection(ReplyCode, String, Identifier, Identifier),
    FinishConnectionShutdown,
    InitConnectionRecovery(Error),
    InitConnectionShutdown(Error),
    RemoveChannel(ChannelId, Error),
    ResetHeartbeat,
    SendConnectionCloseOk(Error),
    SetChannelStatus(ChannelId, KillSwitch),
    SetConnectionClosing,
    SetConnectionClosed(Error),
    SetConnectionError(Error),
    Spawn(Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>),
    StartChannelsRecovery,
    StartHeartbeat(Duration),
}

impl<RK: RuntimeKit + Clone + Send + 'static> InternalRPC<RK> {
    pub(crate) fn new(
        runtime: Runtime<RK>,
        heartbeat: Heartbeat<RK>,
        waker: SocketStateHandle,
    ) -> Self {
        let (sender, rpc) = flume::unbounded();
        let handle = InternalRPCHandle { sender, waker };
        Self {
            rpc,
            handle,
            channels_status: Default::default(),
            heartbeat,
            runtime,
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

    pub(crate) fn register_internal_future(
        &self,
        fut: impl Future<Output = Result<()>> + Send + 'static,
    ) {
        let handle = self.handle();
        self.runtime.spawn(Box::pin(async move {
            if let Err(err) = fut.await {
                handle.set_connection_error(err);
            }
        }));
    }

    pub(crate) fn register_internal_future_with_resolver<T: Send + 'static>(
        &self,
        fut: impl Future<Output = Result<T>> + Send + 'static,
        resolver: PromiseResolver<T>,
    ) {
        self.register_internal_future(async move {
            let res = fut.await;
            resolver.complete(res);
            Ok(())
        });
    }

    pub(crate) fn start(self, channels: Channels) {
        self.runtime.clone().spawn(Box::pin(self.run(channels)));
    }

    async fn run(mut self, channels: Channels) {
        use InternalCommand::*;

        let rpc = self.rpc.clone();
        let get_channel = |id| {
            channels
                .get(id)
                .ok_or::<Error>(ErrorKind::InvalidChannel(id).into())
        };

        while let Ok(Some(command)) = rpc.recv_async().await {
            trace!("Handling internal RPC command"); // FIXME: restore ?command (future is not debug for Spawn)
            match command {
                BasicAck(channel_id, delivery_tag, options, resolver, error) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    self.register_internal_future_with_resolver(
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
                    self.register_internal_future_with_resolver(
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
                    self.register_internal_future_with_resolver(
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
                    self.register_internal_future(async move {
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
                CancelHeartbeat => self.heartbeat.cancel(),
                CloseChannel(channel_id, reply_code, reply_text) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    self.register_internal_future(async move {
                        channel?.close(reply_code, &reply_text).await
                    })
                }
                CloseConnection(reply_code, reply_text, class_id, method_id) => {
                    let channel = channels.channel0();
                    self.register_internal_future(async move {
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
                RemoveChannel(channel_id, error) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channels = channels.clone();
                    self.register_internal_future(async move { channels.remove(channel_id, error) })
                }
                ResetHeartbeat => self.heartbeat.reset(),
                SendConnectionCloseOk(error) => {
                    let channel = channels.channel0();
                    self.register_internal_future(async move {
                        channel.connection_close_ok(error).await
                    })
                }
                SetChannelStatus(channel_id, killswitch) => {
                    self.channels_status.insert(channel_id, killswitch);
                }
                SetConnectionClosing => channels.set_connection_closing(),
                SetConnectionClosed(error) => channels.set_connection_closed(error),
                SetConnectionError(error) => channels.set_connection_error(error),
                Spawn(fut) => self.register_internal_future(fut),
                StartChannelsRecovery => {
                    let channels = channels.clone();
                    self.register_internal_future(async move { channels.start_recovery().await })
                }
                StartHeartbeat(heartbeat) => {
                    self.heartbeat.set_timeout(heartbeat);
                    self.heartbeat.start(channels.clone());
                }
            }
            self.handle.waker.wake();
        }
        trace!("InternalRPC stopped");
    }
}
