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
    secret_update::SecretUpdate,
    shared::SharedMutex,
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
    secret_update: SecretUpdate<RK>,
    runtime: Runtime<RK>,
}

#[derive(Clone)]
pub(crate) struct InternalRPCHandle {
    sender: SharedMutex<Option<Sender<Option<InternalCommand>>>>,
    waker: SocketStateHandle,
}

impl InternalRPCHandle {
    pub(crate) fn basic_ack(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicAckOptions,
        resolver: PromiseResolver<()>,
        poison: ErrorHolder,
    ) {
        self.send(InternalCommand::BasicAck(
            channel_id,
            delivery_tag,
            options,
            DeliveryResolver::new(channel_id, poison, resolver),
        ));
    }

    pub(crate) fn basic_nack(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicNackOptions,
        resolver: PromiseResolver<()>,
        poison: ErrorHolder,
    ) {
        self.send(InternalCommand::BasicNack(
            channel_id,
            delivery_tag,
            options,
            DeliveryResolver::new(channel_id, poison, resolver),
        ));
    }

    pub(crate) fn basic_reject(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicRejectOptions,
        resolver: PromiseResolver<()>,
        poison: ErrorHolder,
    ) {
        self.send(InternalCommand::BasicReject(
            channel_id,
            delivery_tag,
            options,
            DeliveryResolver::new(channel_id, poison, resolver),
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

    pub(crate) fn start_secret_update(&self) {
        self.send(InternalCommand::StartSecretUpdate);
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
        // Detach the shared sender first so later calls are rejected. A send
        // that already cloned it may still arrive after the sentinel; run()
        // rejects those commands until the final sender is dropped.
        let sender = self.sender.lock().take();
        if let Some(sender) = sender {
            let _ = sender.send(None);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sender.lock().as_ref().is_none_or(Sender::is_empty)
    }

    fn send(&self, command: InternalCommand) {
        trace!("Queuing internal RPC command");
        // Sending and sender destruction may wake the dispatcher. Neither
        // may occur while holding the shared sender lock.
        let sender = self.sender.lock().clone();
        if let Some(sender) = sender {
            match sender.send(Some(command)) {
                Ok(()) => self.waker.wake(),
                Err(flume::SendError(Some(command))) => {
                    command.reject_pending(Error::other("internal RPC loop has stopped"));
                }
                Err(flume::SendError(None)) => {}
            }
        } else {
            command.reject_pending(Error::other("internal RPC loop has stopped"));
        }
    }
}

impl fmt::Debug for InternalRPCHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InternalRPCHandle").finish()
    }
}

#[derive(Debug)]
enum InternalCommand {
    BasicAck(ChannelId, DeliveryTag, BasicAckOptions, DeliveryResolver),
    BasicNack(ChannelId, DeliveryTag, BasicNackOptions, DeliveryResolver),
    BasicReject(ChannelId, DeliveryTag, BasicRejectOptions, DeliveryResolver),
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
    StartSecretUpdate,
    UpdateSecret(LongString, ShortString, PromiseResolver<()>),
}

/// The resolver of an ack, nack or reject command, along with the poison of the delivery's
/// acker. Dropping it before its command runs rejects the promise with the error that ended the
/// channel, or with `InvalidChannel` when the channel recorded none. A command that never runs
/// thus still completes the caller's promise. That happens when the command's channel is gone, or
/// when the IO loop has already exited.
#[derive(Debug)]
struct DeliveryResolver {
    channel_id: ChannelId,
    poison: ErrorHolder,
    resolver: Option<PromiseResolver<()>>,
}

impl DeliveryResolver {
    fn new(channel_id: ChannelId, poison: ErrorHolder, resolver: PromiseResolver<()>) -> Self {
        Self {
            channel_id,
            poison,
            resolver: Some(resolver),
        }
    }

    fn into_parts(mut self) -> (ErrorHolder, PromiseResolver<()>) {
        let resolver = self
            .resolver
            .take()
            .expect("the resolver is only taken by into_parts");
        (std::mem::take(&mut self.poison), resolver)
    }
}

impl Drop for DeliveryResolver {
    fn drop(&mut self) {
        if let Some(resolver) = self.resolver.take() {
            let error = self
                .poison
                .check()
                .err()
                .unwrap_or_else(|| ErrorKind::InvalidChannel(self.channel_id).into());
            resolver.reject(error);
        }
    }
}

/// Runs the ack, nack or reject in `settle`, unless `poison` already holds the error that ended
/// the delivery's channel. When `settle` fails after the channel ended, returns that error instead
/// of the one `settle` hit, since the channel ending is what made `settle` fail.
async fn settle_delivery(
    poison: ErrorHolder,
    settle: impl Future<Output = Result<()>>,
) -> Result<()> {
    poison.check()?;
    settle
        .await
        .map_err(|error| poison.check().err().unwrap_or(error))
}

impl InternalCommand {
    /// Rejects the promise this command carries. Ack, nack and reject commands are dropped
    /// instead, so their `DeliveryResolver` reports the error that ended their channel.
    fn reject_pending(self, error: Error) {
        match self {
            Self::CloseConnection(_, _, _, _, Some(resolver))
            | Self::UpdateSecret(_, _, resolver) => resolver.reject(error),
            Self::CreateChannel(_, resolver) => resolver.reject(error),
            Self::InitConnectionShutdown(_, Some(resolver)) => resolver.reject(error),
            _ => {}
        }
    }
}

impl<RK: RuntimeKit + Clone + Send + 'static> InternalRPC<RK> {
    pub(crate) fn new(
        runtime: Runtime<RK>,
        heartbeat: Heartbeat<RK>,
        secret_update: SecretUpdate<RK>,
        frames: Frames,
        waker: SocketStateHandle,
    ) -> Self {
        let (sender, rpc) = flume::unbounded();
        let handle = InternalRPCHandle {
            sender: SharedMutex::new(Some(sender)),
            waker,
        };
        Self {
            rpc,
            handle,
            channels_status: Default::default(),
            frames,
            heartbeat,
            secret_update,
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
        let get_channel = |id| {
            channels
                .get(id)
                .ok_or::<Error>(ErrorKind::InvalidChannel(id).into())
        };

        let mut stopping = false;
        while let Ok(command) = rpc.recv_async().await {
            let Some(command) = command else {
                stopping = true;
                continue;
            };
            if stopping {
                command.reject_pending(Error::other("internal RPC loop has stopped"));
                continue;
            }
            trace!("Handling internal RPC command");
            match command {
                BasicAck(channel_id, delivery_tag, options, resolver) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    let (poison, resolver) = resolver.into_parts();
                    self.register_internal_future_with_resolver(
                        settle_delivery(poison, async move {
                            channel?.basic_ack(delivery_tag, options).await
                        }),
                        resolver,
                    )
                }
                BasicNack(channel_id, delivery_tag, options, resolver) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    let (poison, resolver) = resolver.into_parts();
                    self.register_internal_future_with_resolver(
                        settle_delivery(poison, async move {
                            channel?.basic_nack(delivery_tag, options).await
                        }),
                        resolver,
                    )
                }
                BasicReject(channel_id, delivery_tag, options, resolver) => {
                    if !self.channel_ok(channel_id) {
                        continue;
                    }
                    let channel = get_channel(channel_id);
                    let (poison, resolver) = resolver.into_parts();
                    self.register_internal_future_with_resolver(
                        settle_delivery(poison, async move {
                            channel?.basic_reject(delivery_tag, options).await
                        }),
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
                    let Ok(channel) = get_channel(channel_id) else {
                        continue;
                    };
                    self.register_internal_future(async move {
                        match channel.close(reply_code, reply_text).await {
                            Err(err)
                                if matches!(err.kind(), ErrorKind::InvalidChannelState(..)) =>
                            {
                                Ok(())
                            }
                            result => result,
                        }
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
                    self.secret_update.reset();
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
                    self.secret_update.cancel();
                    channels.set_connection_closing();
                }
                SetConnectionClosed(error) => {
                    self.frames.clear_all_expected_replies(error.clone());
                    channels.set_connection_closed(error);
                }
                SetConnectionError(error) => {
                    if channels.can_recover(&error) {
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
                StartSecretUpdate => {
                    self.secret_update.start(self.handle());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChannelState, ConnectionProperties, ConnectionState, ConnectionStatus,
        configuration::Configuration, events::Events, runtime, socket_state::SocketState,
        uri::AMQPUri,
    };
    use futures_lite::future::block_on;

    fn poisoned(kind: ErrorKind) -> ErrorHolder {
        let poison = ErrorHolder::default();
        poison.set(kind.into()).unwrap();
        poison
    }

    #[test]
    fn a_delivery_command_dropped_unrun_rejects_its_promise() {
        let (promise, resolver) = Promise::new("basic.ack");
        drop(DeliveryResolver::new(1, ErrorHolder::default(), resolver));
        let error = promise.try_wait().expect("promise completed").unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::InvalidChannel(1)));
    }

    #[test]
    fn a_delivery_command_dropped_unrun_reports_its_channel_error() {
        let (promise, resolver) = Promise::new("basic.ack");
        let poison = poisoned(ErrorKind::MissingHeartbeatError);
        drop(DeliveryResolver::new(1, poison, resolver));
        let error = promise.try_wait().expect("promise completed").unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::MissingHeartbeatError));
    }

    #[test]
    fn a_delivery_command_handed_on_leaves_its_promise_to_the_command() {
        let (promise, resolver) = Promise::new("basic.ack");
        let (_, resolver) = DeliveryResolver::new(1, ErrorHolder::default(), resolver).into_parts();
        assert!(promise.try_wait().is_none());
        resolver.resolve(());
        assert!(matches!(promise.try_wait(), Some(Ok(()))));
    }

    #[test]
    fn a_delivery_whose_channel_ended_is_not_settled() {
        let mut settled = false;
        let result = block_on(settle_delivery(
            poisoned(ErrorKind::MissingHeartbeatError),
            async {
                settled = true;
                Ok(())
            },
        ));
        assert!(!settled);
        assert!(matches!(
            result.unwrap_err().kind(),
            ErrorKind::MissingHeartbeatError
        ));
    }

    #[test]
    fn a_delivery_failing_as_its_channel_ends_reports_the_channel_error() {
        let poison = ErrorHolder::default();
        let result = block_on(settle_delivery(poison.clone(), async {
            poison.set(ErrorKind::MissingHeartbeatError.into()).unwrap();
            Err(ErrorKind::InvalidChannelState(ChannelState::Closing, "basic.ack").into())
        }));
        assert!(matches!(
            result.unwrap_err().kind(),
            ErrorKind::MissingHeartbeatError
        ));
    }

    #[test]
    fn a_delivery_failing_on_a_live_channel_reports_its_own_error() {
        let result = block_on(settle_delivery(ErrorHolder::default(), async {
            Err(ErrorKind::InvalidChannel(1).into())
        }));
        assert!(matches!(
            result.unwrap_err().kind(),
            ErrorKind::InvalidChannel(1)
        ));
    }

    fn create_rpc() -> (InternalRPC<runtime::DefaultRuntimeKit>, Channels) {
        let uri = AMQPUri::default();
        let runtime = runtime::default_runtime().unwrap();
        let configuration = Configuration::new(&uri, ConnectionProperties::default());
        let status = ConnectionStatus::new(&uri);
        status.set_state(ConnectionState::Connected);
        let frames = Frames::default();
        let socket_state = SocketState::default();
        let heartbeat = Heartbeat::new(status.clone(), runtime.clone());
        let secret_update = SecretUpdate::new(
            status.clone(),
            runtime.clone(),
            configuration.auth_provider.clone(),
        );
        let rpc = InternalRPC::new(
            runtime,
            heartbeat,
            secret_update,
            frames.clone(),
            socket_state.handle(),
        );
        let channels = Channels::new(
            configuration,
            status.clone(),
            socket_state.handle(),
            rpc.handle(),
            frames,
            Events::new(status),
        );
        (rpc, channels)
    }

    #[test]
    fn stop_rejects_commands_after_its_sentinel() {
        let (rpc, channels) = create_rpc();
        let handle = rpc.handle();
        handle.stop();
        let (promise, resolver) = Promise::new("late connection.update-secret");
        handle.send(InternalCommand::UpdateSecret(
            "secret".into(),
            "reason".into(),
            resolver,
        ));

        block_on(rpc.run(channels));

        assert!(promise.try_wait().unwrap().is_err());
    }

    #[test]
    fn stop_rejects_inflight_commands_after_its_sentinel() {
        let (rpc, channels) = create_rpc();
        let handle = rpc.handle();
        // Simulate a sender preempted after cloning, before queueing its call.
        let sender = handle.sender.lock().as_ref().unwrap().clone();
        handle.stop();
        let (promise, resolver) = Promise::new("inflight connection.update-secret");
        sender
            .send(Some(InternalCommand::UpdateSecret(
                "secret".into(),
                "reason".into(),
                resolver,
            )))
            .unwrap();
        drop(sender);

        block_on(rpc.run(channels));

        assert!(promise.try_wait().unwrap().is_err());
    }

    #[test]
    fn receiver_is_woken_without_holding_sender_lock() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            task::{Context, Wake, Waker},
        };

        struct CheckSenderOnWake {
            handle: InternalRPCHandle,
            blocked: AtomicBool,
            called: AtomicBool,
        }

        impl Wake for CheckSenderOnWake {
            fn wake(self: Arc<Self>) {
                if self.handle.sender.try_lock().is_none() {
                    self.blocked.store(true, Ordering::SeqCst);
                    return;
                }
                self.called.store(true, Ordering::SeqCst);
            }
        }

        let (sender, receiver) = flume::unbounded();
        let handle = InternalRPCHandle {
            sender: SharedMutex::new(Some(sender)),
            waker: SocketState::default().handle(),
        };
        let wake = Arc::new(CheckSenderOnWake {
            handle: handle.clone(),
            blocked: AtomicBool::new(false),
            called: AtomicBool::new(false),
        });
        let waker = Waker::from(wake.clone());
        let mut cx = Context::from_waker(&waker);
        let mut receiving = std::pin::pin!(receiver.recv_async());
        assert!(receiving.as_mut().poll(&mut cx).is_pending());
        let (_promise, resolver) = Promise::new("basic.ack");

        handle.basic_ack(
            1,
            1,
            BasicAckOptions::default(),
            resolver,
            ErrorHolder::default(),
        );

        handle.stop();
        assert!(!wake.blocked.load(Ordering::SeqCst));
        assert!(wake.called.load(Ordering::SeqCst));
        assert!(handle.sender.lock().is_none());
        assert!(receiving.as_mut().poll(&mut cx).is_ready());
    }

    #[test]
    fn stopped_rpc_rejects_pending_command() {
        let (sender, receiver) = flume::unbounded();
        drop(receiver);
        let handle = InternalRPCHandle {
            sender: SharedMutex::new(Some(sender)),
            waker: SocketState::default().handle(),
        };
        let (promise, resolver) = Promise::new("connection.update-secret");
        handle.send(InternalCommand::UpdateSecret(
            "secret".into(),
            "reason".into(),
            resolver,
        ));
        assert!(promise.try_wait().unwrap().is_err());
    }
}
