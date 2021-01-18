use crate::{
    channels::Channels,
    consumer_status::ConsumerStatus,
    options::{BasicAckOptions, BasicCancelOptions, BasicNackOptions, BasicRejectOptions},
    socket_state::SocketStateHandle,
    types::{ChannelId, DeliveryTag, Identifier, ReplyCode},
    Error, PromiseResolver, Result,
};
use executor_trait::FullExecutor;
use flume::{Receiver, Sender};
use std::{fmt, future::Future, sync::Arc};
use tracing::trace;

pub(crate) struct InternalRPC {
    rpc: Receiver<Option<InternalCommand>>,
    handle: InternalRPCHandle,
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
    ) {
        self.send(InternalCommand::BasicAck(
            channel_id,
            delivery_tag,
            options,
            resolver,
        ));
    }

    pub(crate) fn basic_nack(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicNackOptions,
        resolver: PromiseResolver<()>,
    ) {
        self.send(InternalCommand::BasicNack(
            channel_id,
            delivery_tag,
            options,
            resolver,
        ));
    }

    pub(crate) fn basic_reject(
        &self,
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        options: BasicRejectOptions,
        resolver: PromiseResolver<()>,
    ) {
        self.send(InternalCommand::BasicReject(
            channel_id,
            delivery_tag,
            options,
            resolver,
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

    pub(crate) fn send_connection_close_ok(&self, error: Error) {
        self.send(InternalCommand::SendConnectionCloseOk(error));
    }

    pub(crate) fn remove_channel(&self, channel_id: ChannelId, error: Error) {
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

    pub(crate) fn stop(&self) {
        trace!("Stopping internal RPC command");
        let _ = self.sender.send(None);
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
        let internal_rpc = self.clone();
        self.executor.spawn(Box::pin(async move {
            if let Err(err) = f.await {
                internal_rpc.set_connection_error(err);
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
            resolver.swear(res);
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
    BasicAck(ChannelId, DeliveryTag, BasicAckOptions, PromiseResolver<()>),
    BasicNack(
        ChannelId,
        DeliveryTag,
        BasicNackOptions,
        PromiseResolver<()>,
    ),
    BasicReject(
        ChannelId,
        DeliveryTag,
        BasicRejectOptions,
        PromiseResolver<()>,
    ),
    CancelConsumer(ChannelId, String, ConsumerStatus),
    CloseChannel(ChannelId, ReplyCode, String),
    CloseConnection(ReplyCode, String, Identifier, Identifier),
    SendConnectionCloseOk(Error),
    RemoveChannel(ChannelId, Error),
    SetConnectionClosing,
    SetConnectionClosed(Error),
    SetConnectionError(Error),
}

impl InternalRPC {
    pub(crate) fn new(
        executor: Arc<dyn FullExecutor + Send + Sync>,
        waker: SocketStateHandle,
    ) -> Self {
        let (sender, rpc) = flume::unbounded();
        let handle = InternalRPCHandle {
            sender,
            waker,
            executor,
        };
        Self { rpc, handle }
    }

    pub(crate) fn handle(&self) -> InternalRPCHandle {
        self.handle.clone()
    }

    pub(crate) async fn run(self, channels: Channels) {
        use InternalCommand::*;

        while let Ok(Some(command)) = self.rpc.recv_async().await {
            trace!(?command, "Handling internal RPC command");
            let handle = self.handle();
            match command {
                BasicAck(channel_id, delivery_tag, options, resolver) => channels
                    .get(channel_id)
                    .map(|channel| {
                        self.handle.register_internal_future_with_resolver(
                            async move { channel.basic_ack(delivery_tag, options).await },
                            resolver,
                        )
                    })
                    .unwrap_or_default(),
                BasicNack(channel_id, delivery_tag, options, resolver) => channels
                    .get(channel_id)
                    .map(|channel| {
                        self.handle.register_internal_future_with_resolver(
                            async move { channel.basic_nack(delivery_tag, options).await },
                            resolver,
                        )
                    })
                    .unwrap_or_default(),
                BasicReject(channel_id, delivery_tag, options, resolver) => channels
                    .get(channel_id)
                    .map(|channel| {
                        self.handle.register_internal_future_with_resolver(
                            async move { channel.basic_reject(delivery_tag, options).await },
                            resolver,
                        )
                    })
                    .unwrap_or_default(),
                CancelConsumer(channel_id, consumer_tag, consumer_status) => channels
                    .get(channel_id)
                    .map(|channel| {
                        if channel.status().connected() && consumer_status.state().is_active() {
                            handle.register_internal_future(async move {
                                channel
                                    .basic_cancel(&consumer_tag, BasicCancelOptions::default())
                                    .await
                            })
                        }
                    })
                    .unwrap_or_default(),
                CloseChannel(channel_id, reply_code, reply_text) => channels
                    .get(channel_id)
                    .map(|channel| {
                        handle.register_internal_future(async move {
                            channel.close(reply_code, &reply_text).await
                        })
                    })
                    .unwrap_or_default(),
                CloseConnection(reply_code, reply_text, class_id, method_id) => channels
                    .get(0)
                    .map(move |channel0| {
                        handle.register_internal_future(async move {
                            channel0
                                .connection_close(reply_code, &reply_text, class_id, method_id)
                                .await
                        })
                    })
                    .unwrap_or_default(),
                SendConnectionCloseOk(error) => channels
                    .get(0)
                    .map(move |channel| {
                        handle.register_internal_future(async move {
                            channel.connection_close_ok(error).await
                        })
                    })
                    .unwrap_or_default(),
                RemoveChannel(channel_id, error) => {
                    let channels = channels.clone();
                    handle
                        .register_internal_future(async move { channels.remove(channel_id, error) })
                }
                SetConnectionClosing => channels.set_connection_closing(),
                SetConnectionClosed(error) => channels.set_connection_closed(error),
                SetConnectionError(error) => channels.set_connection_error(error),
            }
            self.handle.waker.wake();
        }
        trace!("InternalRPC stopped");
    }
}
