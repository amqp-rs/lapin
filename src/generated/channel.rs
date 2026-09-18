/// Options structs for every AMQP method that accepts flag arguments.
pub mod options {
    use super::*;

    /// Options for the `basic.qos` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicQosOptions {
        /// Apply quality-of-service settings globally to the entire connection rather than just this channel.
        pub global: Boolean,
    }

    /// Options for the `basic.consume` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicConsumeOptions {
        /// Do not receive messages published by this connection on this consumer.
        pub no_local: Boolean,
        /// Disable message acknowledgement; the server dequeues messages as soon as they are delivered.
        pub no_ack: Boolean,
        /// Restrict access to the declaring connection; the entity is deleted when that connection closes.
        pub exclusive: Boolean,
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `basic.cancel` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicCancelOptions {
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `basic.publish` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicPublishOptions {
        /// Return the message to the publisher if it cannot be routed to at least one queue.
        pub mandatory: Boolean,
        /// Return the message to the publisher if no consumer is immediately available to receive it.
        pub immediate: Boolean,
    }

    /// Options for the `basic.deliver` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicDeliverOptions {
        /// Indicates the message was previously delivered but not acknowledged.
        pub redelivered: Boolean,
    }

    /// Options for the `basic.get` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicGetOptions {
        /// Disable message acknowledgement; the server dequeues messages as soon as they are delivered.
        pub no_ack: Boolean,
    }

    /// Options for the `basic.get-ok` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicGetOkOptions {
        /// Indicates the message was previously delivered but not acknowledged.
        pub redelivered: Boolean,
    }

    /// Options for the `basic.ack` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicAckOptions {
        /// Acknowledge or reject all outstanding deliveries up to and including this delivery tag.
        pub multiple: Boolean,
    }

    /// Options for the `basic.reject` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicRejectOptions {
        /// Re-queue the message rather than discarding or dead-lettering it.
        pub requeue: Boolean,
    }

    /// Options for the `basic.recover-async` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicRecoverAsyncOptions {
        /// Re-queue the message rather than discarding or dead-lettering it.
        pub requeue: Boolean,
    }

    /// Options for the `basic.recover` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicRecoverOptions {
        /// Re-queue the message rather than discarding or dead-lettering it.
        pub requeue: Boolean,
    }

    /// Options for the `basic.nack` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicNackOptions {
        /// Acknowledge or reject all outstanding deliveries up to and including this delivery tag.
        pub multiple: Boolean,
        /// Re-queue the message rather than discarding or dead-lettering it.
        pub requeue: Boolean,
    }

    /// Options for the `channel.flow` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ChannelFlowOptions {
        /// Enable or disable message flow on the channel.
        pub active: Boolean,
    }

    /// Options for the `channel.flow-ok` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ChannelFlowOkOptions {
        /// Enable or disable message flow on the channel.
        pub active: Boolean,
    }

    /// Options for the `access.request` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct AccessRequestOptions {
        /// Restrict access to the declaring connection; the entity is deleted when that connection closes.
        pub exclusive: Boolean,
        /// Verify that the exchange or queue exists without creating or modifying it.
        pub passive: Boolean,
        /// Enable or disable message flow on the channel.
        pub active: Boolean,
        /// Request write permission for the resource (access class only).
        pub write: Boolean,
        /// Request read permission for the resource (access class only).
        pub read: Boolean,
    }

    /// Options for the `exchange.declare` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeDeclareOptions {
        /// Verify that the exchange or queue exists without creating or modifying it.
        pub passive: Boolean,
        /// Survive broker restarts; the entity is persisted to disk.
        pub durable: Boolean,
        /// Delete the exchange or queue automatically when it has no consumers or bindings.
        pub auto_delete: Boolean,
        /// Mark the exchange as internal; clients may not publish directly to it.
        pub internal: Boolean,
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `exchange.delete` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeDeleteOptions {
        /// Only delete the exchange or queue if it has no consumers or bindings.
        pub if_unused: Boolean,
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `exchange.bind` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeBindOptions {
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `exchange.unbind` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeUnbindOptions {
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `queue.declare` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueueDeclareOptions {
        /// Verify that the exchange or queue exists without creating or modifying it.
        pub passive: Boolean,
        /// Survive broker restarts; the entity is persisted to disk.
        pub durable: Boolean,
        /// Restrict access to the declaring connection; the entity is deleted when that connection closes.
        pub exclusive: Boolean,
        /// Delete the exchange or queue automatically when it has no consumers or bindings.
        pub auto_delete: Boolean,
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `queue.bind` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueueBindOptions {
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `queue.purge` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueuePurgeOptions {
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `queue.delete` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueueDeleteOptions {
        /// Only delete the exchange or queue if it has no consumers or bindings.
        pub if_unused: Boolean,
        /// Only delete the queue if it has no messages.
        pub if_empty: Boolean,
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }

    /// Options for the `confirm.select` AMQP method.
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ConfirmSelectOptions {
        /// Do not wait for a server confirmation; the operation is fire-and-forget.
        pub nowait: Boolean,
    }
}

use options::*;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Reply {
    ConnectionStep(ConnectionStep),
    BasicQosOk(PromiseResolver<()>),
    BasicConsumeOk(
        PromiseResolver<Consumer>,
        Option<Arc<ChannelCloser>>,
        ShortString,
        BasicConsumeOptions,
        FieldTable,
        Option<Consumer>,
    ),
    BasicCancelOk(PromiseResolver<()>),
    BasicGetOk(PromiseResolver<Option<BasicGetMessage>>),
    BasicRecoverOk(PromiseResolver<()>),
    ConnectionCloseOk(PromiseResolver<()>),
    ConnectionUpdateSecretOk(PromiseResolver<()>),
    ChannelOpenOk(PromiseResolver<Channel>, Channel),
    ChannelFlowOk(PromiseResolver<Boolean>),
    ChannelCloseOk(PromiseResolver<()>),
    AccessRequestOk(PromiseResolver<()>),
    ExchangeDeclareOk(
        PromiseResolver<()>,
        ShortString,
        ExchangeKind,
        ExchangeDeclareOptions,
        FieldTable,
    ),
    ExchangeDeleteOk(PromiseResolver<()>, ShortString),
    ExchangeBindOk(
        PromiseResolver<()>,
        ShortString,
        ShortString,
        ShortString,
        FieldTable,
    ),
    ExchangeUnbindOk(
        PromiseResolver<()>,
        ShortString,
        ShortString,
        ShortString,
        FieldTable,
    ),
    QueueDeclareOk(PromiseResolver<Queue>, QueueDeclareOptions, FieldTable),
    QueueBindOk(
        PromiseResolver<()>,
        ShortString,
        ShortString,
        ShortString,
        FieldTable,
    ),
    QueuePurgeOk(PromiseResolver<MessageCount>),
    QueueDeleteOk(PromiseResolver<MessageCount>, ShortString),
    QueueUnbindOk(
        PromiseResolver<()>,
        ShortString,
        ShortString,
        ShortString,
        FieldTable,
    ),
    TxSelectOk(PromiseResolver<()>),
    TxCommitOk(PromiseResolver<()>),
    TxRollbackOk(PromiseResolver<()>),
    ConfirmSelectOk(PromiseResolver<()>),
}

impl Channel {
    pub(crate) fn receive_method(&self, method: AMQPClass) -> Result<()> {
        match method {
            AMQPClass::Basic(protocol::basic::AMQPMethod::QosOk(m)) => self.receive_basic_qos_ok(m),
            AMQPClass::Basic(protocol::basic::AMQPMethod::ConsumeOk(m)) => {
                self.receive_basic_consume_ok(m)
            }
            AMQPClass::Basic(protocol::basic::AMQPMethod::Cancel(m)) => {
                self.receive_basic_cancel(m)
            }
            AMQPClass::Basic(protocol::basic::AMQPMethod::CancelOk(m)) => {
                self.receive_basic_cancel_ok(m)
            }
            AMQPClass::Basic(protocol::basic::AMQPMethod::Return(m)) => {
                self.receive_basic_return(m)
            }
            AMQPClass::Basic(protocol::basic::AMQPMethod::Deliver(m)) => {
                self.receive_basic_deliver(m)
            }
            AMQPClass::Basic(protocol::basic::AMQPMethod::GetOk(m)) => self.receive_basic_get_ok(m),
            AMQPClass::Basic(protocol::basic::AMQPMethod::GetEmpty(m)) => {
                self.receive_basic_get_empty(m)
            }
            AMQPClass::Basic(protocol::basic::AMQPMethod::Ack(m)) => self.receive_basic_ack(m),
            AMQPClass::Basic(protocol::basic::AMQPMethod::RecoverOk(m)) => {
                self.receive_basic_recover_ok(m)
            }
            AMQPClass::Basic(protocol::basic::AMQPMethod::Nack(m)) => self.receive_basic_nack(m),
            AMQPClass::Connection(protocol::connection::AMQPMethod::Start(m)) => {
                self.receive_connection_start(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::Secure(m)) => {
                self.receive_connection_secure(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::Tune(m)) => {
                self.receive_connection_tune(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::OpenOk(m)) => {
                self.receive_connection_open_ok(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::Close(m)) => {
                self.receive_connection_close(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::CloseOk(m)) => {
                self.receive_connection_close_ok(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::Blocked(m)) => {
                self.receive_connection_blocked(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::Unblocked(m)) => {
                self.receive_connection_unblocked(m)
            }
            AMQPClass::Connection(protocol::connection::AMQPMethod::UpdateSecretOk(m)) => {
                self.receive_connection_update_secret_ok(m)
            }
            AMQPClass::Channel(protocol::channel::AMQPMethod::OpenOk(m)) => {
                self.receive_channel_open_ok(m)
            }
            AMQPClass::Channel(protocol::channel::AMQPMethod::Flow(m)) => {
                self.receive_channel_flow(m)
            }
            AMQPClass::Channel(protocol::channel::AMQPMethod::FlowOk(m)) => {
                self.receive_channel_flow_ok(m)
            }
            AMQPClass::Channel(protocol::channel::AMQPMethod::Close(m)) => {
                self.receive_channel_close(m)
            }
            AMQPClass::Channel(protocol::channel::AMQPMethod::CloseOk(m)) => {
                self.receive_channel_close_ok(m)
            }
            AMQPClass::Access(protocol::access::AMQPMethod::RequestOk(m)) => {
                self.receive_access_request_ok(m)
            }
            AMQPClass::Exchange(protocol::exchange::AMQPMethod::DeclareOk(m)) => {
                self.receive_exchange_declare_ok(m)
            }
            AMQPClass::Exchange(protocol::exchange::AMQPMethod::DeleteOk(m)) => {
                self.receive_exchange_delete_ok(m)
            }
            AMQPClass::Exchange(protocol::exchange::AMQPMethod::BindOk(m)) => {
                self.receive_exchange_bind_ok(m)
            }
            AMQPClass::Exchange(protocol::exchange::AMQPMethod::UnbindOk(m)) => {
                self.receive_exchange_unbind_ok(m)
            }
            AMQPClass::Queue(protocol::queue::AMQPMethod::DeclareOk(m)) => {
                self.receive_queue_declare_ok(m)
            }
            AMQPClass::Queue(protocol::queue::AMQPMethod::BindOk(m)) => {
                self.receive_queue_bind_ok(m)
            }
            AMQPClass::Queue(protocol::queue::AMQPMethod::PurgeOk(m)) => {
                self.receive_queue_purge_ok(m)
            }
            AMQPClass::Queue(protocol::queue::AMQPMethod::DeleteOk(m)) => {
                self.receive_queue_delete_ok(m)
            }
            AMQPClass::Queue(protocol::queue::AMQPMethod::UnbindOk(m)) => {
                self.receive_queue_unbind_ok(m)
            }
            AMQPClass::Tx(protocol::tx::AMQPMethod::SelectOk(m)) => self.receive_tx_select_ok(m),
            AMQPClass::Tx(protocol::tx::AMQPMethod::CommitOk(m)) => self.receive_tx_commit_ok(m),
            AMQPClass::Tx(protocol::tx::AMQPMethod::RollbackOk(m)) => {
                self.receive_tx_rollback_ok(m)
            }
            AMQPClass::Confirm(protocol::confirm::AMQPMethod::SelectOk(m)) => {
                self.receive_confirm_select_ok(m)
            }
            m => {
                error!(method=?m, "The client should not receive this method");
                self.handle_invalid_contents(
                    format!("unexpected method received on channel {}", self.id),
                    m.get_amqp_class_id(),
                    m.get_amqp_method_id(),
                )
            }
        }
    }

    /// Set the quality of service for this channel.
    ///
    /// Limits the number of unacknowledged messages that the server will deliver.
    /// `prefetch_count` controls the maximum number of unacknowledged messages;
    /// 0 means no limit. The `global` flag (in [`BasicQosOptions`]) determines
    /// whether the limit applies per-consumer (`false`) or across the whole channel
    /// (`true`).
    ///
    /// Call this before [`Channel::basic_consume`] to control back-pressure.
    pub async fn basic_qos(
        &self,
        prefetch_count: ShortUInt,
        options: BasicQosOptions,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.qos"));
        }

        let BasicQosOptions { global } = options;
        let (promise, resolver) = Promise::new("basic.qos");
        let reply = Reply::BasicQosOk(resolver.clone());
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Qos(protocol::basic::Qos {
            prefetch_count,
            global,
        }));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_basic_qos_ok(&self, method: protocol::basic::QosOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.qos-ok"));
        }

        match self
            .frames
            .find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::BasicQosOk(..)))
        {
            Some(Reply::BasicQosOk(resolver)) => fwd_res(Ok(()), Some(resolver)),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected basic qos-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Register a consumer on a queue.
    ///
    /// Use the higher-level [`Channel::basic_consume`] wrapper instead of calling
    /// this method directly.
    ///
    /// [`Channel::basic_consume`]: crate::Channel::basic_consume
    async fn do_basic_consume(
        &self,
        queue: ShortString,
        consumer_tag: ShortString,
        options: BasicConsumeOptions,
        arguments: FieldTable,
        original: Option<Consumer>,
    ) -> Result<Consumer> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.consume"));
        }

        let creation_arguments = arguments.clone();
        let BasicConsumeOptions {
            no_local,
            no_ack,
            exclusive,
            nowait,
        } = options;
        let (promise, resolver) = Promise::new("basic.consume");
        let reply = Reply::BasicConsumeOk(
            resolver.clone(),
            self.channel_closer.clone(),
            queue.clone(),
            options,
            creation_arguments,
            original,
        );
        let nowait_reply = nowait.then(|| protocol::basic::ConsumeOk {
            consumer_tag: consumer_tag.clone(),
        });
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Consume(
            protocol::basic::Consume {
                queue,
                consumer_tag,
                no_local,
                no_ack,
                exclusive,
                nowait,
                arguments,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_basic_consume_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_basic_consume_ok(&self, method: protocol::basic::ConsumeOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.consume-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::BasicConsumeOk(..))
        }) {
            Some(Reply::BasicConsumeOk(
                resolver,
                channel_closer,
                queue,
                options,
                creation_arguments,
                original,
            )) => fwd_res(
                self.on_basic_consume_ok_received(
                    method,
                    resolver,
                    channel_closer,
                    queue,
                    options,
                    creation_arguments,
                    original,
                ),
                None,
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected basic consume-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Cancel a consumer subscription.
    ///
    /// Tells the server to stop delivering messages for the consumer identified by
    /// `consumer_tag`. This is the counterpart to [`Channel::basic_consume`]. After
    /// this call the consumer's stream will end with `None`.
    ///
    /// Prefer calling this over simply dropping the [`Consumer`]: an explicit cancel
    /// sends the cancellation through the server so that no further messages are
    /// delivered, whereas a drop only discards already-delivered ones.
    pub async fn basic_cancel(
        &self,
        consumer_tag: ShortString,
        options: BasicCancelOptions,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.cancel"));
        }

        self.before_basic_cancel(consumer_tag.as_str());
        let BasicCancelOptions { nowait } = options;
        let (promise, resolver) = Promise::new("basic.cancel");
        let reply = Reply::BasicCancelOk(resolver.clone());
        let nowait_reply = nowait.then(|| protocol::basic::CancelOk {
            consumer_tag: consumer_tag.clone(),
        });
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Cancel(
            protocol::basic::Cancel {
                consumer_tag,
                nowait,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_basic_cancel_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_basic_cancel(&self, method: protocol::basic::Cancel) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.cancel"));
        }
        self.on_basic_cancel_received(method)
    }
    /// Server confirmation that the consumer identified by the consumer tag has been cancelled.
    async fn basic_cancel_ok(&self, consumer_tag: ShortString) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.cancel-ok"));
        }

        let (promise, resolver) = Promise::new("basic.cancel-ok");
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::CancelOk(
            protocol::basic::CancelOk { consumer_tag },
        ));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        promise.await
    }
    fn receive_basic_cancel_ok(&self, method: protocol::basic::CancelOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.cancel-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::BasicCancelOk(..))
        }) {
            Some(Reply::BasicCancelOk(resolver)) => {
                fwd_res(self.on_basic_cancel_ok_received(method), Some(resolver))
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected basic cancel-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Publish a message to an exchange.
    ///
    /// Routes `payload` through `exchange` using `routing_key`. The empty string
    /// `""` selects the default exchange, which routes messages directly to the
    /// queue whose name matches `routing_key`.
    ///
    /// `properties` carries AMQP message metadata (content-type, headers,
    /// delivery-mode, priority, …). Use [`BasicProperties::default()`] when you
    /// do not need to set any.
    ///
    /// Returns a [`crate::PublisherConfirm`] future. If publisher confirms are **not**
    /// enabled (via [`Channel::confirm_select`]) the future resolves immediately
    /// to [`crate::Confirmation::NotRequested`]. If they are enabled, it resolves once
    /// the broker acknowledges or negatively-acknowledges the message.
    ///
    /// Use [`Channel::wait_for_confirms`] to drain all outstanding confirms at
    /// once.
    pub async fn basic_publish(
        &self,
        exchange: ShortString,
        routing_key: ShortString,
        options: BasicPublishOptions,
        payload: &[u8],
        properties: BasicProperties,
    ) -> Result<PublisherConfirm> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.publish"));
        }

        let start_hook_res = self.before_basic_publish();
        let BasicPublishOptions {
            mandatory,
            immediate,
        } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Publish(
            protocol::basic::Publish {
                exchange,
                routing_key,
                mandatory,
                immediate,
            },
        ));

        self.send_method_frame_with_body(
            "basic.publish",
            method,
            payload,
            properties,
            start_hook_res,
        )
        .await
    }
    fn receive_basic_return(&self, method: protocol::basic::Return) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.return"));
        }
        self.on_basic_return_received(method)
    }
    fn receive_basic_deliver(&self, method: protocol::basic::Deliver) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.deliver"));
        }
        self.on_basic_deliver_received(method)
    }
    /// Synchronously poll a single message from a queue.
    ///
    /// Use the higher-level [`Channel::basic_get`] wrapper instead of calling
    /// this method directly.
    ///
    /// [`Channel::basic_get`]: crate::Channel::basic_get
    async fn do_basic_get(
        &self,
        queue: ShortString,
        options: BasicGetOptions,
        original: Option<PromiseResolver<Option<BasicGetMessage>>>,
    ) -> Result<Option<BasicGetMessage>> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.get"));
        }

        let BasicGetOptions { no_ack } = options;
        let (promise, resolver) = Promise::new("basic.get");
        let reply = Reply::BasicGetOk(resolver.clone());
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Get(protocol::basic::Get {
            queue,
            no_ack,
        }));

        let resolver = original.unwrap_or(resolver);
        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_basic_get_ok(&self, method: protocol::basic::GetOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.get-ok"));
        }

        match self
            .frames
            .find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::BasicGetOk(..)))
        {
            Some(Reply::BasicGetOk(resolver)) => {
                fwd_res(self.on_basic_get_ok_received(method, resolver), None)
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected basic get-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    fn receive_basic_get_empty(&self, method: protocol::basic::GetEmpty) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.get-empty"));
        }
        self.on_basic_get_empty_received(method)
    }
    /// Acknowledge one or more messages.
    ///
    /// Tells the server that the consumer has successfully processed the message
    /// identified by `delivery_tag`. If [`BasicAckOptions::multiple`] is `true`,
    /// all unacknowledged messages up to and including `delivery_tag` are
    /// acknowledged in one shot.
    ///
    /// Prefer using [`crate::Acker::ack`] on the delivery directly; this low-level
    /// method is exposed for advanced use-cases.
    pub async fn basic_ack(
        &self,
        delivery_tag: LongLongUInt,
        options: BasicAckOptions,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.ack"));
        }

        let BasicAckOptions { multiple } = options;
        let (promise, resolver) = Promise::new("basic.ack");
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Ack(protocol::basic::Ack {
            delivery_tag,
            multiple,
        }));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        self.on_basic_ack_sent(multiple, delivery_tag);
        promise.await
    }
    fn receive_basic_ack(&self, method: protocol::basic::Ack) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.ack"));
        }
        self.on_basic_ack_received(method)
    }
    /// Reject a single message.
    ///
    /// Signals to the server that the consumer could not process the message
    /// identified by `delivery_tag`. If [`BasicRejectOptions::requeue`] is `true`
    /// the message is re-queued; otherwise it is discarded or sent to a dead-letter
    /// exchange.
    ///
    /// To reject multiple messages in one call, use [`Channel::basic_nack`] with
    /// [`BasicNackOptions::multiple`] set to `true`.
    ///
    /// Prefer using [`crate::Acker::reject`] on the delivery directly; this low-level
    /// method is exposed for advanced use-cases.
    pub async fn basic_reject(
        &self,
        delivery_tag: LongLongUInt,
        options: BasicRejectOptions,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.reject"));
        }

        let BasicRejectOptions { requeue } = options;
        let (promise, resolver) = Promise::new("basic.reject");
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Reject(
            protocol::basic::Reject {
                delivery_tag,
                requeue,
            },
        ));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        promise.await
    }
    /// Ask the server to redeliver all unacknowledged messages (fire-and-forget).
    ///
    /// This is the asynchronous variant of [`Channel::basic_recover`]: it sends
    /// the request but does not wait for a broker confirmation. If
    /// [`BasicRecoverAsyncOptions::requeue`] is `true` the messages may be
    /// delivered to a different consumer; if `false` they are redelivered to the
    /// original consumer.
    ///
    /// Prefer [`Channel::basic_recover`] unless you specifically need
    /// fire-and-forget semantics.
    pub async fn basic_recover_async(&self, options: BasicRecoverAsyncOptions) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.recover-async"));
        }

        let BasicRecoverAsyncOptions { requeue } = options;
        let (promise, resolver) = Promise::new("basic.recover-async");
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::RecoverAsync(
            protocol::basic::RecoverAsync { requeue },
        ));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        self.on_basic_recover_async_sent();
        promise.await
    }
    /// Ask the server to redeliver all unacknowledged messages.
    ///
    /// Waits for the broker to confirm that all outstanding unacknowledged
    /// messages have been requeued or redelivered. If
    /// [`BasicRecoverOptions::requeue`] is `true` the messages may be delivered
    /// to a different consumer; if `false` they are redelivered to the original
    /// consumer.
    pub async fn basic_recover(&self, options: BasicRecoverOptions) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.recover"));
        }

        let BasicRecoverOptions { requeue } = options;
        let (promise, resolver) = Promise::new("basic.recover");
        let reply = Reply::BasicRecoverOk(resolver.clone());
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Recover(
            protocol::basic::Recover { requeue },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_basic_recover_ok(&self, method: protocol::basic::RecoverOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.recover-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::BasicRecoverOk(..))
        }) {
            Some(Reply::BasicRecoverOk(resolver)) => {
                fwd_res(self.on_basic_recover_ok_received(), Some(resolver))
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected basic recover-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Negatively acknowledge one or more messages (RabbitMQ extension).
    ///
    /// Similar to [`Channel::basic_reject`] but supports bulk rejection via
    /// [`BasicNackOptions::multiple`]. If `multiple` is `true`, all
    /// unacknowledged messages up to and including `delivery_tag` are rejected.
    /// If [`BasicNackOptions::requeue`] is `true` the messages are re-queued;
    /// otherwise they are discarded or dead-lettered.
    ///
    /// Prefer using [`crate::Acker::nack`] on the delivery directly; this low-level
    /// method is exposed for advanced use-cases.
    pub async fn basic_nack(
        &self,
        delivery_tag: LongLongUInt,
        options: BasicNackOptions,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("basic.nack"));
        }

        let BasicNackOptions { multiple, requeue } = options;
        let (promise, resolver) = Promise::new("basic.nack");
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Nack(protocol::basic::Nack {
            delivery_tag,
            multiple,
            requeue,
        }));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        self.on_basic_nack_sent(multiple, delivery_tag);
        promise.await
    }
    fn receive_basic_nack(&self, method: protocol::basic::Nack) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("basic.nack"));
        }
        self.on_basic_nack_received(method)
    }
    fn receive_connection_start(&self, method: protocol::connection::Start) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.start"));
        }

        match self.frames.find_connection_step(self.id) {
            Some(step) => self.on_connection_start_received(method, step),
            None => self.connection_process_error(self.connection_status.state(), None, None),
        }
    }
    /// Client response to `connection.start`, providing the chosen security mechanism and initial credentials.
    async fn connection_start_ok(
        &self,
        client_properties: FieldTable,
        mechanism: ShortString,
        response: LongString,
        locale: ShortString,
        conn_resolver: PromiseResolver<Connection>,
        connection: Connection,
        auth_provider: Arc<dyn AuthProvider>,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.start-ok");
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::StartOk(
            protocol::connection::StartOk {
                client_properties,
                mechanism,
                response,
                locale,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(
                Reply::ConnectionStep(ConnectionStep::StartOk(
                    conn_resolver,
                    connection,
                    auth_provider,
                )),
                Box::new(resolver),
            )),
            None,
        );
        promise.await
    }
    fn receive_connection_secure(&self, method: protocol::connection::Secure) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.secure"));
        }

        match self.frames.find_connection_step(self.id) {
            Some(step) => self.on_connection_secure_received(method, step),
            None => self.connection_process_error(self.connection_status.state(), None, None),
        }
    }
    /// Client response to a `connection.secure` challenge, providing the SASL response data.
    async fn connection_secure_ok(
        &self,
        response: LongString,
        conn_resolver: PromiseResolver<Connection>,
        connection: Connection,
        auth_provider: Arc<dyn AuthProvider>,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.secure-ok");
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::SecureOk(
            protocol::connection::SecureOk { response },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(
                Reply::ConnectionStep(ConnectionStep::SecureOk(
                    conn_resolver,
                    connection,
                    auth_provider,
                )),
                Box::new(resolver),
            )),
            None,
        );
        promise.await
    }
    fn receive_connection_tune(&self, method: protocol::connection::Tune) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.tune"));
        }

        match self.frames.find_connection_step(self.id) {
            Some(step) => self.on_connection_tune_received(method, step),
            None => self.connection_process_error(self.connection_status.state(), None, None),
        }
    }
    /// Client confirmation of negotiated connection parameters (channel max, frame max, heartbeat).
    async fn connection_tune_ok(
        &self,
        channel_max: ShortUInt,
        frame_max: LongUInt,
        heartbeat: ShortUInt,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.tune-ok");
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::TuneOk(
            protocol::connection::TuneOk {
                channel_max,
                frame_max,
                heartbeat,
            },
        ));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        promise.await
    }
    /// Client request to open a virtual host, completing the connection handshake.
    pub(crate) async fn connection_open(
        &self,
        virtual_host: ShortString,
        conn_resolver: PromiseResolver<Connection>,
        connection: Connection,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.open");
        let reply = Reply::ConnectionStep(ConnectionStep::Open(conn_resolver, connection));
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::Open(
            protocol::connection::Open { virtual_host },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_connection_open_ok(&self, method: protocol::connection::OpenOk) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.open-ok"));
        }

        match self.frames.find_connection_step(self.id) {
            Some(ConnectionStep::Open(conn_resolver, connection)) => fwd_res(
                self.on_connection_open_ok_received(method, connection, conn_resolver),
                None,
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected connection open-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Request to close the connection, providing a reply code and text.
    ///
    /// Either side may initiate a close. The peer must reply with
    /// `connection.close-ok` before the TCP connection is torn down.
    pub(crate) async fn connection_close(
        &self,
        reply_code: ShortUInt,
        reply_text: ShortString,
        class_id: ShortUInt,
        method_id: ShortUInt,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.close");
        let reply = Reply::ConnectionCloseOk(resolver.clone());
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::Close(
            protocol::connection::Close {
                reply_code,
                reply_text,
                class_id,
                method_id,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_connection_close(&self, method: protocol::connection::Close) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.close"));
        }
        self.on_connection_close_received(method)
    }
    /// Acknowledgement of a `connection.close` request; the TCP connection may now be closed.
    pub(crate) async fn connection_close_ok(&self, error: Error) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.close-ok");
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::CloseOk(
            protocol::connection::CloseOk {},
        ));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        self.on_connection_close_ok_sent(error);
        promise.await
    }
    fn receive_connection_close_ok(&self, method: protocol::connection::CloseOk) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.close-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ConnectionCloseOk(..))
        }) {
            Some(Reply::ConnectionCloseOk(resolver)) => {
                fwd_res(self.on_connection_close_ok_received(), Some(resolver))
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected connection close-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    fn receive_connection_blocked(&self, method: protocol::connection::Blocked) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.blocked"));
        }
        self.on_connection_blocked_received(method)
    }
    fn receive_connection_unblocked(&self, method: protocol::connection::Unblocked) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.unblocked"));
        }
        self.on_connection_unblocked_received(method)
    }
    /// Request the server to update the authentication secret (e.g. rotate an OAuth2 token).
    ///
    /// Use [`Connection::update_secret`] or [`auth::TokenAuthProvider`] for automatic rotation.
    ///
    /// [`Connection::update_secret`]: crate::Connection::update_secret
    /// [`auth::TokenAuthProvider`]: crate::auth::TokenAuthProvider
    pub(crate) async fn connection_update_secret(
        &self,
        new_secret: LongString,
        reason: ShortString,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.update-secret");
        let reply = Reply::ConnectionUpdateSecretOk(resolver.clone());
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::UpdateSecret(
            protocol::connection::UpdateSecret { new_secret, reason },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_connection_update_secret_ok(
        &self,
        method: protocol::connection::UpdateSecretOk,
    ) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.update-secret-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::ConnectionUpdateSecretOk(..))){
      Some(Reply::ConnectionUpdateSecretOk(resolver)) => {
        fwd_res(Ok(()), Some(resolver))
      },
      unexpected => {
        self.handle_invalid_contents(format!("unexpected connection update-secret-ok received on channel {}, was awaiting for {:?}", self.id, unexpected), method.get_amqp_class_id(), method.get_amqp_method_id())
      },
    }
    }
    /// Open a new channel on the connection, identified by its channel number.
    pub(crate) async fn channel_open(&self, channel: Channel) -> Result<Channel> {
        if !self.status.initializing() {
            return Err(self.status.state_error("channel.open"));
        }

        let (promise, resolver) = Promise::new("channel.open");
        let reply = Reply::ChannelOpenOk(resolver.clone(), channel);
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::Open(
            protocol::channel::Open {},
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_channel_open_ok(&self, method: protocol::channel::OpenOk) -> Result<()> {
        if !self.status.initializing() {
            return Err(self.status.state_error("channel.open-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ChannelOpenOk(..))
        }) {
            Some(Reply::ChannelOpenOk(resolver, channel)) => fwd_res(
                self.on_channel_open_ok_received(method, resolver, channel),
                None,
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected channel open-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Enable or disable message flow on this channel.
    ///
    /// When [`ChannelFlowOptions::active`] is `false`, the server stops sending
    /// content frames. This is a flow-control mechanism: pause delivery when your
    /// consumer is overwhelmed and resume it when ready. Returns the actual flow
    /// state confirmed by the server.
    ///
    /// Note: RabbitMQ supports this method but does not apply back-pressure on
    /// the publisher side; prefer [`Channel::basic_qos`] for consumer-side
    /// throttling.
    pub async fn channel_flow(&self, options: ChannelFlowOptions) -> Result<Boolean> {
        if !self.status.connected() {
            return Err(self.status.state_error("channel.flow"));
        }

        let ChannelFlowOptions { active } = options;
        let (promise, resolver) = Promise::new("channel.flow");
        let reply = Reply::ChannelFlowOk(resolver.clone());
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::Flow(
            protocol::channel::Flow { active },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_channel_flow(&self, method: protocol::channel::Flow) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("channel.flow"));
        }
        self.on_channel_flow_received(method)
    }
    /// Server confirmation of the current message flow state on the channel.
    async fn channel_flow_ok(&self, options: ChannelFlowOkOptions) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("channel.flow-ok"));
        }

        let ChannelFlowOkOptions { active } = options;
        let (promise, resolver) = Promise::new("channel.flow-ok");
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::FlowOk(
            protocol::channel::FlowOk { active },
        ));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        promise.await
    }
    fn receive_channel_flow_ok(&self, method: protocol::channel::FlowOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("channel.flow-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ChannelFlowOk(..))
        }) {
            Some(Reply::ChannelFlowOk(resolver)) => {
                fwd_res(self.on_channel_flow_ok_received(method, resolver), None)
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected channel flow-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Request to close the channel, providing a reply code and text.
    ///
    /// The peer must respond with `channel.close-ok`. Use [`Channel::close`] rather
    /// than calling this directly.
    ///
    /// [`Channel::close`]: crate::Channel::close
    async fn do_channel_close(
        &self,
        reply_code: ShortUInt,
        reply_text: ShortString,
        class_id: ShortUInt,
        method_id: ShortUInt,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("channel.close"));
        }

        self.before_channel_close();
        let (promise, resolver) = Promise::new("channel.close");
        let reply = Reply::ChannelCloseOk(resolver.clone());
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::Close(
            protocol::channel::Close {
                reply_code,
                reply_text,
                class_id,
                method_id,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_channel_close(&self, method: protocol::channel::Close) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("channel.close"));
        }
        self.on_channel_close_received(method)
    }
    /// Acknowledgement of a `channel.close` request; the channel is now closed.
    async fn channel_close_ok(&self, error: Option<Error>) -> Result<()> {
        if !self.status.closing() {
            return Err(self.status.state_error("channel.close-ok"));
        }

        let (promise, resolver) = Promise::new("channel.close-ok");
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::CloseOk(
            protocol::channel::CloseOk {},
        ));

        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        self.on_channel_close_ok_sent(error);
        promise.await
    }
    fn receive_channel_close_ok(&self, method: protocol::channel::CloseOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("channel.close-ok"));
        }

        match self.next_expected_close_ok_reply() {
            Some(Reply::ChannelCloseOk(resolver)) => {
                fwd_res(self.on_channel_close_ok_received(), Some(resolver))
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected channel close-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Request access to a virtual host (AMQP 0-8 compatibility).
    ///
    /// This method is a no-op in RabbitMQ and is retained only for compatibility
    /// with AMQP 0-8 brokers. In AMQP 0-9-1 access control is handled at
    /// connection time. The returned ticket value is ignored by modern brokers.
    pub async fn access_request(
        &self,
        realm: ShortString,
        options: AccessRequestOptions,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("access.request"));
        }

        let AccessRequestOptions {
            exclusive,
            passive,
            active,
            write,
            read,
        } = options;
        let (promise, resolver) = Promise::new("access.request");
        let reply = Reply::AccessRequestOk(resolver.clone());
        let method = AMQPClass::Access(protocol::access::AMQPMethod::Request(
            protocol::access::Request {
                realm,
                exclusive,
                passive,
                active,
                write,
                read,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_access_request_ok(&self, method: protocol::access::RequestOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("access.request-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::AccessRequestOk(..))
        }) {
            Some(Reply::AccessRequestOk(resolver)) => {
                fwd_res(self.on_access_request_ok_received(method), Some(resolver))
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected access request-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Declare an exchange, creating it if it does not already exist.
    ///
    /// Use the higher-level [`Channel::exchange_declare`] wrapper instead of
    /// calling this method directly.
    ///
    /// [`Channel::exchange_declare`]: crate::Channel::exchange_declare
    async fn do_exchange_declare(
        &self,
        exchange: ShortString,
        kind: ShortString,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
        exchange_kind: ExchangeKind,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("exchange.declare"));
        }

        let creation_arguments = arguments.clone();
        let ExchangeDeclareOptions {
            passive,
            durable,
            auto_delete,
            internal,
            nowait,
        } = options;
        let (promise, resolver) = Promise::new("exchange.declare");
        let reply = Reply::ExchangeDeclareOk(
            resolver.clone(),
            exchange.clone(),
            exchange_kind,
            options,
            creation_arguments,
        );
        let nowait_reply = nowait.then_some(protocol::exchange::DeclareOk {});
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Declare(
            protocol::exchange::Declare {
                exchange,
                kind,
                passive,
                durable,
                auto_delete,
                internal,
                nowait,
                arguments,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_exchange_declare_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_exchange_declare_ok(&self, method: protocol::exchange::DeclareOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("exchange.declare-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ExchangeDeclareOk(..))
        }) {
            Some(Reply::ExchangeDeclareOk(
                resolver,
                exchange,
                exchange_kind,
                options,
                creation_arguments,
            )) => fwd_res(
                self.on_exchange_declare_ok_received(
                    resolver,
                    exchange,
                    exchange_kind,
                    options,
                    creation_arguments,
                ),
                None,
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected exchange declare-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Delete an exchange
    pub async fn exchange_delete(
        &self,
        exchange: ShortString,
        options: ExchangeDeleteOptions,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("exchange.delete"));
        }

        let ExchangeDeleteOptions { if_unused, nowait } = options;
        let (promise, resolver) = Promise::new("exchange.delete");
        let reply = Reply::ExchangeDeleteOk(resolver.clone(), exchange.clone());
        let nowait_reply = nowait.then_some(protocol::exchange::DeleteOk {});
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Delete(
            protocol::exchange::Delete {
                exchange,
                if_unused,
                nowait,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_exchange_delete_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_exchange_delete_ok(&self, method: protocol::exchange::DeleteOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("exchange.delete-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ExchangeDeleteOk(..))
        }) {
            Some(Reply::ExchangeDeleteOk(resolver, exchange)) => fwd_res(
                self.on_exchange_delete_ok_received(exchange),
                Some(resolver),
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected exchange delete-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Bind a source exchange to a destination exchange.
    ///
    /// Messages published to `source` that match `routing_key` (and `arguments`
    /// for header exchanges) will be forwarded to `destination`. This is a
    /// RabbitMQ extension that allows exchange-to-exchange routing.
    ///
    /// The binding is removed with [`Channel::exchange_unbind`].
    pub async fn exchange_bind(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        options: ExchangeBindOptions,
        arguments: FieldTable,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("exchange.bind"));
        }

        let creation_arguments = arguments.clone();
        let ExchangeBindOptions { nowait } = options;
        let (promise, resolver) = Promise::new("exchange.bind");
        let reply = Reply::ExchangeBindOk(
            resolver.clone(),
            destination.clone(),
            source.clone(),
            routing_key.clone(),
            creation_arguments,
        );
        let nowait_reply = nowait.then_some(protocol::exchange::BindOk {});
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Bind(
            protocol::exchange::Bind {
                destination,
                source,
                routing_key,
                nowait,
                arguments,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_exchange_bind_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_exchange_bind_ok(&self, method: protocol::exchange::BindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("exchange.bind-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ExchangeBindOk(..))
        }) {
            Some(Reply::ExchangeBindOk(
                resolver,
                destination,
                source,
                routing_key,
                creation_arguments,
            )) => fwd_res(
                self.on_exchange_bind_ok_received(
                    destination,
                    source,
                    routing_key,
                    creation_arguments,
                ),
                Some(resolver),
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected exchange bind-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Remove a binding between two exchanges.
    ///
    /// Removes the exchange-to-exchange binding created by [`Channel::exchange_bind`].
    /// `source`, `destination`, `routing_key`, and `arguments` must exactly match
    /// the parameters used when the binding was created.
    pub async fn exchange_unbind(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        options: ExchangeUnbindOptions,
        arguments: FieldTable,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("exchange.unbind"));
        }

        let creation_arguments = arguments.clone();
        let ExchangeUnbindOptions { nowait } = options;
        let (promise, resolver) = Promise::new("exchange.unbind");
        let reply = Reply::ExchangeUnbindOk(
            resolver.clone(),
            destination.clone(),
            source.clone(),
            routing_key.clone(),
            creation_arguments,
        );
        let nowait_reply = nowait.then_some(protocol::exchange::UnbindOk {});
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Unbind(
            protocol::exchange::Unbind {
                destination,
                source,
                routing_key,
                nowait,
                arguments,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_exchange_unbind_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_exchange_unbind_ok(&self, method: protocol::exchange::UnbindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("exchange.unbind-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ExchangeUnbindOk(..))
        }) {
            Some(Reply::ExchangeUnbindOk(
                resolver,
                destination,
                source,
                routing_key,
                creation_arguments,
            )) => fwd_res(
                self.on_exchange_unbind_ok_received(
                    destination,
                    source,
                    routing_key,
                    creation_arguments,
                ),
                Some(resolver),
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected exchange unbind-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Declare a queue, creating it if it does not already exist.
    ///
    /// Returns a [`Queue`] value carrying the queue name, message count, and
    /// consumer count as reported by the server.
    ///
    /// Common option presets are available as constructor methods on
    /// [`QueueDeclareOptions`]: [`QueueDeclareOptions::durable`],
    /// [`QueueDeclareOptions::exclusive`].
    ///
    /// When [`QueueDeclareOptions::passive`] is `true` the server only checks
    /// whether the queue exists without modifying it; an error is returned if it
    /// does not.
    pub async fn queue_declare(
        &self,
        queue: ShortString,
        options: QueueDeclareOptions,
        arguments: FieldTable,
    ) -> Result<Queue> {
        if !self.status.connected() {
            return Err(self.status.state_error("queue.declare"));
        }

        let creation_arguments = arguments.clone();
        let QueueDeclareOptions {
            passive,
            durable,
            exclusive,
            auto_delete,
            nowait,
        } = options;
        let (promise, resolver) = Promise::new("queue.declare");
        let reply = Reply::QueueDeclareOk(resolver.clone(), options, creation_arguments);
        let nowait_reply = nowait.then(|| protocol::queue::DeclareOk {
            queue: queue.clone(),
            ..Default::default()
        });
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Declare(
            protocol::queue::Declare {
                queue,
                passive,
                durable,
                exclusive,
                auto_delete,
                nowait,
                arguments,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_queue_declare_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_queue_declare_ok(&self, method: protocol::queue::DeclareOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("queue.declare-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::QueueDeclareOk(..))
        }) {
            Some(Reply::QueueDeclareOk(resolver, options, creation_arguments)) => fwd_res(
                self.on_queue_declare_ok_received(method, resolver, options, creation_arguments),
                None,
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected queue declare-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Bind a queue to an exchange.
    ///
    /// Messages published to `exchange` that match `routing_key` (and `arguments`
    /// for header exchanges) will be routed to `queue`. Multiple bindings with
    /// different routing keys can be created between the same queue and exchange.
    ///
    /// The binding is removed with [`Channel::queue_unbind`].
    pub async fn queue_bind(
        &self,
        queue: ShortString,
        exchange: ShortString,
        routing_key: ShortString,
        options: QueueBindOptions,
        arguments: FieldTable,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("queue.bind"));
        }

        let creation_arguments = arguments.clone();
        let QueueBindOptions { nowait } = options;
        let (promise, resolver) = Promise::new("queue.bind");
        let reply = Reply::QueueBindOk(
            resolver.clone(),
            queue.clone(),
            exchange.clone(),
            routing_key.clone(),
            creation_arguments,
        );
        let nowait_reply = nowait.then_some(protocol::queue::BindOk {});
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Bind(protocol::queue::Bind {
            queue,
            exchange,
            routing_key,
            nowait,
            arguments,
        }));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_queue_bind_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_queue_bind_ok(&self, method: protocol::queue::BindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("queue.bind-ok"));
        }

        match self
            .frames
            .find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::QueueBindOk(..)))
        {
            Some(Reply::QueueBindOk(
                resolver,
                queue,
                exchange,
                routing_key,
                creation_arguments,
            )) => fwd_res(
                self.on_queue_bind_ok_received(queue, exchange, routing_key, creation_arguments),
                Some(resolver),
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected queue bind-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Delete all messages from a queue without deleting the queue itself.
    ///
    /// Returns the number of messages that were purged. This operation is
    /// irreversible; use with care.
    pub async fn queue_purge(
        &self,
        queue: ShortString,
        options: QueuePurgeOptions,
    ) -> Result<MessageCount> {
        if !self.status.connected() {
            return Err(self.status.state_error("queue.purge"));
        }

        let QueuePurgeOptions { nowait } = options;
        let (promise, resolver) = Promise::new("queue.purge");
        let reply = Reply::QueuePurgeOk(resolver.clone());
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Purge(protocol::queue::Purge {
            queue,
            nowait,
        }));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_queue_purge_ok(&self, method: protocol::queue::PurgeOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("queue.purge-ok"));
        }

        match self
            .frames
            .find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::QueuePurgeOk(..)))
        {
            Some(Reply::QueuePurgeOk(resolver)) => {
                fwd_res(self.on_queue_purge_ok_received(method, resolver), None)
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected queue purge-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Delete a queue.
    ///
    /// Returns the number of messages that were in the queue. The queue and all
    /// its bindings are removed. If [`QueueDeleteOptions::if_unused`] is set, the
    /// delete only succeeds when there are no consumers; if
    /// [`QueueDeleteOptions::if_empty`] is set, it only succeeds when the queue
    /// has no messages.
    pub async fn queue_delete(
        &self,
        queue: ShortString,
        options: QueueDeleteOptions,
    ) -> Result<MessageCount> {
        if !self.status.connected() {
            return Err(self.status.state_error("queue.delete"));
        }

        let QueueDeleteOptions {
            if_unused,
            if_empty,
            nowait,
        } = options;
        let (promise, resolver) = Promise::new("queue.delete");
        let reply = Reply::QueueDeleteOk(resolver.clone(), queue.clone());
        let nowait_reply = nowait.then_some(protocol::queue::DeleteOk {
            ..Default::default()
        });
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Delete(
            protocol::queue::Delete {
                queue,
                if_unused,
                if_empty,
                nowait,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        if let Some(nowait_reply) = nowait_reply {
            self.receive_queue_delete_ok(nowait_reply)?;
        }
        promise.await
    }
    fn receive_queue_delete_ok(&self, method: protocol::queue::DeleteOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("queue.delete-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::QueueDeleteOk(..))
        }) {
            Some(Reply::QueueDeleteOk(resolver, queue)) => fwd_res(
                self.on_queue_delete_ok_received(method, resolver, queue),
                None,
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected queue delete-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Remove a binding between a queue and an exchange.
    ///
    /// Removes the binding created by [`Channel::queue_bind`]. `queue`,
    /// `exchange`, `routing_key`, and `arguments` must exactly match the
    /// parameters used when the binding was created.
    pub async fn queue_unbind(
        &self,
        queue: ShortString,
        exchange: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("queue.unbind"));
        }

        let creation_arguments = arguments.clone();
        let (promise, resolver) = Promise::new("queue.unbind");
        let reply = Reply::QueueUnbindOk(
            resolver.clone(),
            queue.clone(),
            exchange.clone(),
            routing_key.clone(),
            creation_arguments,
        );
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Unbind(
            protocol::queue::Unbind {
                queue,
                exchange,
                routing_key,
                arguments,
            },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_queue_unbind_ok(&self, method: protocol::queue::UnbindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("queue.unbind-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::QueueUnbindOk(..))
        }) {
            Some(Reply::QueueUnbindOk(
                resolver,
                queue,
                exchange,
                routing_key,
                creation_arguments,
            )) => fwd_res(
                self.on_queue_unbind_ok_received(queue, exchange, routing_key, creation_arguments),
                Some(resolver),
            ),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected queue unbind-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Enable standard AMQP transactions on this channel.
    ///
    /// Once selected, publishes and acknowledgements are grouped into atomic
    /// transactions that are committed with [`Channel::tx_commit`] or rolled back
    /// with [`Channel::tx_rollback`]. Transactions significantly reduce throughput;
    /// prefer publisher confirms ([`Channel::confirm_select`]) when possible.
    pub async fn tx_select(&self) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("tx.select"));
        }

        let (promise, resolver) = Promise::new("tx.select");
        let reply = Reply::TxSelectOk(resolver.clone());
        let method = AMQPClass::Tx(protocol::tx::AMQPMethod::Select(protocol::tx::Select {}));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_tx_select_ok(&self, method: protocol::tx::SelectOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("tx.select-ok"));
        }

        match self
            .frames
            .find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::TxSelectOk(..)))
        {
            Some(Reply::TxSelectOk(resolver)) => fwd_res(Ok(()), Some(resolver)),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected tx select-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Commit the current transaction.
    ///
    /// All publishes and acknowledgements issued since the last
    /// [`Channel::tx_select`], [`Channel::tx_commit`], or
    /// [`Channel::tx_rollback`] are made permanent. Requires transaction mode to
    /// be enabled first via [`Channel::tx_select`].
    pub async fn tx_commit(&self) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("tx.commit"));
        }

        let (promise, resolver) = Promise::new("tx.commit");
        let reply = Reply::TxCommitOk(resolver.clone());
        let method = AMQPClass::Tx(protocol::tx::AMQPMethod::Commit(protocol::tx::Commit {}));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_tx_commit_ok(&self, method: protocol::tx::CommitOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("tx.commit-ok"));
        }

        match self
            .frames
            .find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::TxCommitOk(..)))
        {
            Some(Reply::TxCommitOk(resolver)) => fwd_res(Ok(()), Some(resolver)),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected tx commit-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Roll back the current transaction.
    ///
    /// Discards all publishes and acknowledgements issued since the last
    /// [`Channel::tx_select`], [`Channel::tx_commit`], or
    /// [`Channel::tx_rollback`]. Requires transaction mode to be enabled first
    /// via [`Channel::tx_select`].
    pub async fn tx_rollback(&self) -> Result<()> {
        if !self.status.connected() {
            return Err(self.status.state_error("tx.rollback"));
        }

        let (promise, resolver) = Promise::new("tx.rollback");
        let reply = Reply::TxRollbackOk(resolver.clone());
        let method = AMQPClass::Tx(protocol::tx::AMQPMethod::Rollback(
            protocol::tx::Rollback {},
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_tx_rollback_ok(&self, method: protocol::tx::RollbackOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("tx.rollback-ok"));
        }

        match self
            .frames
            .find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::TxRollbackOk(..)))
        {
            Some(Reply::TxRollbackOk(resolver)) => fwd_res(Ok(()), Some(resolver)),
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected tx rollback-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    /// Enable publisher confirms on this channel.
    ///
    /// After calling this method, every [`Channel::basic_publish`] call returns a
    /// [`crate::PublisherConfirm`] future that resolves once the broker has either
    /// acknowledged ([`crate::Confirmation::Ack`]) or negatively acknowledged
    /// ([`crate::Confirmation::Nack`]) the message.
    ///
    /// Publisher confirms and AMQP transactions ([`Channel::tx_select`]) are
    /// mutually exclusive. Use [`Channel::wait_for_confirms`] to drain all
    /// outstanding confirms at once.
    pub async fn confirm_select(&self, options: ConfirmSelectOptions) -> Result<()> {
        if !self.status.connected_or_recovering() {
            return Err(self.status.state_error("confirm.select"));
        }

        let ConfirmSelectOptions { nowait } = options;
        let (promise, resolver) = Promise::new("confirm.select");
        let reply = Reply::ConfirmSelectOk(resolver.clone());
        let method = AMQPClass::Confirm(protocol::confirm::AMQPMethod::Select(
            protocol::confirm::Select { nowait },
        ));

        self.send_method_frame(
            method,
            Box::new(resolver.clone()),
            Some(ExpectedReply(reply, Box::new(resolver))),
            None,
        );
        promise.await
    }
    fn receive_confirm_select_ok(&self, method: protocol::confirm::SelectOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("confirm.select-ok"));
        }

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ConfirmSelectOk(..))
        }) {
            Some(Reply::ConfirmSelectOk(resolver)) => {
                fwd_res(self.on_confirm_select_ok_received(), Some(resolver))
            }
            unexpected => self.handle_invalid_contents(
                format!(
                    "unexpected confirm select-ok received on channel {}, was awaiting for {:?}",
                    self.id, unexpected
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
}

fn fwd_res(res: Result<()>, resolver: Option<PromiseResolver<()>>) -> Result<()> {
    if let Some(resolver) = resolver {
        resolver.complete(res.clone());
    }
    res
}
