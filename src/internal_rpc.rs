use crate::{
    Channel, Connection, Error, ErrorKind, Promise, PromiseResolver, Result,
    channels::Channels,
    connection_closer::ConnectionCloser,
    consumer_status::ConsumerStatus,
    error_holder::ErrorHolder,
    frames::Frames,
    future::InternalFuture,
    heartbeat::Heartbeat,
    killswitch::KillSwitch,
    options::{BasicAckOptions, BasicCancelOptions, BasicNackOptions, BasicRejectOptions},
    socket_state::SocketStateHandle,
    types::{ChannelId, DeliveryTag, Identifier, LongString, ReplyCode, ShortString},
};
use amq_protocol::frame::AMQPFrame;
use async_rs::{Runtime, traits::*};
use flume::{Receiver, Sender};
use std::{collections::HashMap, fmt, future::Future, sync::Arc, time::Duration};
use tracing::{debug, trace};

pub(crate) struct InternalRPC<RK: RuntimeKit + Clone + Send + 'static> {
    rpc: Receiver<Option<InternalCommand>>,
    handle: InternalRPCHandle,
    channels_status: HashMap<ChannelId, KillSwitch>,
    frames: Frames,
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
        consumer_tag: ShortString,
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
        reply_text: ShortString,
    ) {
        self.send(InternalCommand::CloseChannel(
            channel_id, reply_code, reply_text,
        ));
    }

    pub(crate) fn close_connection(
        &self,
        reply_code: ReplyCode,
        reply_text: ShortString,
        class_id: Identifier,
        method_id: Identifier,
    ) {
        self.set_connection_closing();
        self.send(InternalCommand::CloseConnection(
            reply_code, reply_text, class_id, method_id, None,
        ));
    }

    pub(crate) async fn close_connection_checked(
        &self,
        reply_code: ReplyCode,
        reply_text: ShortString,
        class_id: Identifier,
        method_id: Identifier,
    ) -> Result<()> {
        self.set_connection_closing();
        let (promise, resolver) = Promise::new("connection.close");
        self.send(InternalCommand::CloseConnection(
            reply_code,
            reply_text,
            class_id,
            method_id,
            Some(resolver),
        ));
        promise.await
    }

    pub(crate) async fn create_channel(
        &self,
        connection_closer: Arc<ConnectionCloser>,
    ) -> Result<Channel> {
        let (promise, resolver) = Promise::new("channel.create");
        self.send(InternalCommand::CreateChannel(connection_closer, resolver));
        promise.await
    }

    pub(crate) fn deregister_consumer(&self, channel_id: ChannelId, consumer_tag: ShortString) {
        self.send(InternalCommand::DeregisterConsumer(
            channel_id,
            consumer_tag,
        ));
    }

    pub(crate) fn finish_connection_shutdown(&self) {
        self.send(InternalCommand::FinishConnectionShutdown);
    }

    pub(crate) fn init_connection_recovery(&self, error: Error) {
        self.send(InternalCommand::InitConnectionRecovery(error));
    }

    pub(crate) fn init_connection_shutdown(
        &self,
        error: Error,
        connection_resolver: Option<PromiseResolver<Connection>>,
    ) {
        self.set_connection_closing();
        self.send(InternalCommand::InitConnectionShutdown(
            error,
            connection_resolver,
        ));
    }

    pub(crate) fn remove_channel(&self, channel_id: ChannelId, error: Error) {
        self.send(InternalCommand::RemoveChannel(channel_id, error));
    }

    pub(crate) fn send_connection_close_ok(&self, error: Error) {
        self.send(InternalCommand::SendConnectionCloseOk(error));
    }

    pub(crate) fn send_heartbeat(&self) {
        self.send(InternalCommand::SendHeartbeat);
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
        self.send(InternalCommand::Spawn(InternalFuture(Box::pin(f))));
    }

    pub(crate) fn spawn_infallible(&self, f: impl Future<Output = ()> + Send + 'static) {
        self.spawn(async move {
            f.await;
            Ok(())
        });
    }

    pub(crate) fn start_channels_recovery(&self) {
        self.send(InternalCommand::StartChannelsRecovery);
    }

    pub(crate) fn start_heartbeat(&self, heartbeat: Duration) {
        self.send(InternalCommand::StartHeartbeat(heartbeat));
    }

    pub(crate) async fn update_secret(
        &self,
        secret: LongString,
        reason: ShortString,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.update-secret");
        self.send(InternalCommand::UpdateSecret(secret, reason, resolver));
        promise.await
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
    CancelConsumer(ChannelId, ShortString, ConsumerStatus),
    CloseChannel(ChannelId, ReplyCode, ShortString),
    CloseConnection(
        ReplyCode,
        ShortString,
        Identifier,
        Identifier,
        Option<PromiseResolver<()>>,
    ),
    CreateChannel(Arc<ConnectionCloser>, PromiseResolver<Channel>),
    DeregisterConsumer(ChannelId, ShortString),
    FinishConnectionShutdown,
    InitConnectionRecovery(Error),
    InitConnectionShutdown(Error, Option<PromiseResolver<Connection>>),
    RemoveChannel(ChannelId, Error),
    SendConnectionCloseOk(Error),
    SendHeartbeat,
    SetChannelStatus(ChannelId, KillSwitch),
    SetConnectionClosing,
    SetConnectionClosed(Error),
    SetConnectionError(Error),
    Spawn(InternalFuture),
    StartChannelsRecovery,
    StartHeartbeat(Duration),
    UpdateSecret(LongString, ShortString, PromiseResolver<()>),
}

impl<RK: RuntimeKit + Clone + Send + 'static> InternalRPC<RK> {
    pub(crate) fn new(
        runtime: Runtime<RK>,
        heartbeat: Heartbeat<RK>,
        frames: Frames,
        waker: SocketStateHandle,
    ) -> Self {
        let (sender, rpc) = flume::unbounded();
        let handle = InternalRPCHandle { sender, waker };
        Self {
            rpc,
            handle,
            channels_status: Default::default(),
            frames,
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
        self.runtime.spawn(async move {
            if let Err(err) = fut.await {
                handle.set_connection_error(err);
            }
        });
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
        self.runtime.clone().spawn(self.run(channels));
    }

    async fn run(mut self, channels: Channels) {
        use InternalCommand::*;

        let rpc = self.rpc.clone();
        let recovery_config = channels.recovery_config();
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
                                .basic_cancel(consumer_tag, BasicCancelOptions::default())
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
                    self.register_internal_future(async move {
                        channel?.close(reply_code, reply_text).await
                    })
                }
                CloseConnection(reply_code, reply_text, class_id, method_id, resolver) => {
                    let channel0 = channels.channel0();
                    let fut = async move {
                        channel0
                            .connection_close(reply_code, reply_text, class_id, method_id)
                            .await
                    };
                    if let Some(resolver) = resolver {
                        self.register_internal_future_with_resolver(fut, resolver)
                    } else {
                        self.register_internal_future(fut)
                    }
                }
                CreateChannel(closer, resolver) => match channels.create(closer) {
                    Ok(channel) => self.register_internal_future_with_resolver(
                        async move { channel.clone().channel_open(channel).await },
                        resolver,
                    ),
                    Err(err) => resolver.reject(err),
                },
                DeregisterConsumer(channel_id, consumer_tag) => {
                    if let Ok(channel) = get_channel(channel_id) {
                        channel.deregister_consumer(consumer_tag.as_str());
                    }
                }
                FinishConnectionShutdown => channels.finish_connection_shutdown(),
                InitConnectionRecovery(error) => {
                    self.heartbeat.reset();
                    channels.init_connection_recovery(error);
                }
                InitConnectionShutdown(error, connection_resolver) => {
                    channels.init_connection_shutdown(error, connection_resolver)
                }
                RemoveChannel(channel_id, error) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channels = channels.clone();
                    self.register_internal_future(async move { channels.remove(channel_id, error) })
                }
                SendConnectionCloseOk(error) => {
                    let channel = channels.channel0();
                    self.register_internal_future(async move {
                        channel.connection_close_ok(error).await
                    })
                }
                SendHeartbeat => {
                    debug!("send heartbeat");
                    let (promise, resolver) = Promise::new("Heartbeat");
                    channels.channel0().send_frame(
                        AMQPFrame::Heartbeat,
                        Box::new(resolver.clone()),
                        None,
                        Some(resolver),
                    );
                    self.register_internal_future(promise);
                }
                SetChannelStatus(channel_id, killswitch) => {
                    self.channels_status.insert(channel_id, killswitch);
                }
                SetConnectionClosing => {
                    self.heartbeat.cancel();
                    channels.set_connection_closing();
                }
                SetConnectionClosed(error) => {
                    self.frames.clear_all_expected_replies(error.clone());
                    channels.set_connection_closed(error);
                }
                SetConnectionError(error) => {
                    if recovery_config.can_recover_connection(&error) {
                        if channels.should_init_connection_recovery() {
                            self.handle.init_connection_recovery(error);
                        }
                    } else {
                        channels.set_connection_error(error)
                    }
                }
                Spawn(fut) => self.register_internal_future(fut),
                StartChannelsRecovery => {
                    let channels = channels.clone();
                    self.register_internal_future(async move { channels.start_recovery().await })
                }
                StartHeartbeat(heartbeat) => {
                    self.heartbeat.set_timeout(heartbeat);
                    self.heartbeat.start(self.handle());
                }
                UpdateSecret(secret, reason, resolver) => {
                    let channels = channels.clone();
                    self.register_internal_future_with_resolver(
                        async move {
                            channels
                                .channel0()
                                .connection_update_secret(secret, reason)
                                .await
                        },
                        resolver,
                    )
                }
            }
            self.handle.waker.wake();
        }
        trace!("InternalRPC stopped");
    }
}
