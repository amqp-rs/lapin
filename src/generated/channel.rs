pub mod options {
    use super::*;

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicQosOptions {
        pub global: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicConsumeOptions {
        pub no_local: Boolean,
        pub no_ack: Boolean,
        pub exclusive: Boolean,
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicCancelOptions {
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicPublishOptions {
        pub mandatory: Boolean,
        pub immediate: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicDeliverOptions {
        pub redelivered: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicGetOptions {
        pub no_ack: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicGetOkOptions {
        pub redelivered: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicAckOptions {
        pub multiple: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicRejectOptions {
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicRecoverAsyncOptions {
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicRecoverOptions {
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct BasicNackOptions {
        pub multiple: Boolean,
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ChannelFlowOptions {
        pub active: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ChannelFlowOkOptions {
        pub active: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct AccessRequestOptions {
        pub exclusive: Boolean,
        pub passive: Boolean,
        pub active: Boolean,
        pub write: Boolean,
        pub read: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeDeclareOptions {
        pub passive: Boolean,
        pub durable: Boolean,
        pub auto_delete: Boolean,
        pub internal: Boolean,
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeDeleteOptions {
        pub if_unused: Boolean,
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeBindOptions {
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ExchangeUnbindOptions {
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueueDeclareOptions {
        pub passive: Boolean,
        pub durable: Boolean,
        pub exclusive: Boolean,
        pub auto_delete: Boolean,
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueueBindOptions {
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueuePurgeOptions {
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct QueueDeleteOptions {
        pub if_unused: Boolean,
        pub if_empty: Boolean,
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct ConfirmSelectOptions {
        pub nowait: Boolean,
    }
}

use options::*;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Reply {
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
    ConnectionOpenOk(PromiseResolver<()>, Box<Connection>),
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
            Some(Reply::BasicQosOk(resolver)) => {
                let res = Ok(());
                resolver.complete(res.clone());
                res
            }
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
            )) => self.on_basic_consume_ok_received(
                method,
                resolver,
                channel_closer,
                queue,
                options,
                creation_arguments,
                original,
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
                let res = self.on_basic_cancel_ok_received(method);
                resolver.complete(res.clone());
                res
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
            Some(Reply::BasicGetOk(resolver)) => self.on_basic_get_ok_received(method, resolver),
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
                let res = self.on_basic_recover_ok_received();
                resolver.complete(res.clone());
                res
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
        self.on_connection_start_received(method)
    }
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

        self.before_connection_start_ok(conn_resolver, connection, auth_provider);
        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        promise.await
    }
    fn receive_connection_secure(&self, method: protocol::connection::Secure) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.secure"));
        }
        self.on_connection_secure_received(method)
    }
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

        self.before_connection_secure_ok(conn_resolver, connection, auth_provider);
        self.send_method_frame(method, Box::new(resolver.clone()), None, Some(resolver));
        promise.await
    }
    fn receive_connection_tune(&self, method: protocol::connection::Tune) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(self.status.state_error("connection.tune"));
        }
        self.on_connection_tune_received(method)
    }
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
    pub(crate) async fn connection_open(
        &self,
        virtual_host: ShortString,
        connection: Box<Connection>,
        conn_resolver: PromiseResolver<Connection>,
    ) -> Result<()> {
        let (promise, resolver) = Promise::new("connection.open");
        let reply = Reply::ConnectionOpenOk(resolver.clone(), connection);
        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::Open(
            protocol::connection::Open { virtual_host },
        ));

        self.before_connection_open(conn_resolver);
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

        match self.frames.find_expected_reply(self.id, |reply| {
            matches!(&reply.0, Reply::ConnectionOpenOk(..))
        }) {
            Some(Reply::ConnectionOpenOk(resolver, connection)) => {
                let res = self.on_connection_open_ok_received(method, connection);
                resolver.complete(res.clone());
                res
            }
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
                let res = self.on_connection_close_ok_received();
                resolver.complete(res.clone());
                res
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
        let res =        Ok(())
;
        resolver.complete(res.clone());
        res
},
      unexpected => {
        self.handle_invalid_contents(format!("unexpected connection update-secret-ok received on channel {}, was awaiting for {:?}", self.id, unexpected), method.get_amqp_class_id(), method.get_amqp_method_id())
      },
    }
    }
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
            Some(Reply::ChannelOpenOk(resolver, channel)) => {
                self.on_channel_open_ok_received(method, resolver, channel)
            }
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
                self.on_channel_flow_ok_received(method, resolver)
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
                let res = self.on_channel_close_ok_received();
                resolver.complete(res.clone());
                res
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
                let res = self.on_access_request_ok_received(method);
                resolver.complete(res.clone());
                res
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
            )) => self.on_exchange_declare_ok_received(
                resolver,
                exchange,
                exchange_kind,
                options,
                creation_arguments,
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
            Some(Reply::ExchangeDeleteOk(resolver, exchange)) => {
                let res = self.on_exchange_delete_ok_received(exchange);
                resolver.complete(res.clone());
                res
            }
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
            )) => {
                let res = self.on_exchange_bind_ok_received(
                    destination,
                    source,
                    routing_key,
                    creation_arguments,
                );
                resolver.complete(res.clone());
                res
            }
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
            )) => {
                let res = self.on_exchange_unbind_ok_received(
                    destination,
                    source,
                    routing_key,
                    creation_arguments,
                );
                resolver.complete(res.clone());
                res
            }
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
            Some(Reply::QueueDeclareOk(resolver, options, creation_arguments)) => {
                self.on_queue_declare_ok_received(method, resolver, options, creation_arguments)
            }
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
            )) => {
                let res = self.on_queue_bind_ok_received(
                    queue,
                    exchange,
                    routing_key,
                    creation_arguments,
                );
                resolver.complete(res.clone());
                res
            }
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
                self.on_queue_purge_ok_received(method, resolver)
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
            Some(Reply::QueueDeleteOk(resolver, queue)) => {
                self.on_queue_delete_ok_received(method, resolver, queue)
            }
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
            )) => {
                let res = self.on_queue_unbind_ok_received(
                    queue,
                    exchange,
                    routing_key,
                    creation_arguments,
                );
                resolver.complete(res.clone());
                res
            }
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
            Some(Reply::TxSelectOk(resolver)) => {
                let res = Ok(());
                resolver.complete(res.clone());
                res
            }
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
            Some(Reply::TxCommitOk(resolver)) => {
                let res = Ok(());
                resolver.complete(res.clone());
                res
            }
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
            Some(Reply::TxRollbackOk(resolver)) => {
                let res = Ok(());
                resolver.complete(res.clone());
                res
            }
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
                let res = self.on_confirm_select_ok_received();
                resolver.complete(res.clone());
                res
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
