pub mod options {
    use super::*;

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct ChannelFlowOptions {
        #[serde(default)]
        pub active: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct ChannelFlowOkOptions {
        #[serde(default)]
        pub active: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct AccessRequestOptions {
        #[serde(default)]
        pub exclusive: Boolean,
        #[serde(default)]
        pub passive: Boolean,
        #[serde(default)]
        pub active: Boolean,
        #[serde(default)]
        pub write: Boolean,
        #[serde(default)]
        pub read: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct ExchangeDeclareOptions {
        #[serde(default)]
        pub passive: Boolean,
        #[serde(default)]
        pub durable: Boolean,
        #[serde(default)]
        pub auto_delete: Boolean,
        #[serde(default)]
        pub internal: Boolean,
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct ExchangeDeleteOptions {
        #[serde(default)]
        pub if_unused: Boolean,
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct ExchangeBindOptions {
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct ExchangeUnbindOptions {
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct QueueDeclareOptions {
        #[serde(default)]
        pub passive: Boolean,
        #[serde(default)]
        pub durable: Boolean,
        #[serde(default)]
        pub exclusive: Boolean,
        #[serde(default)]
        pub auto_delete: Boolean,
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct QueueBindOptions {
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct QueuePurgeOptions {
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct QueueDeleteOptions {
        #[serde(default)]
        pub if_unused: Boolean,
        #[serde(default)]
        pub if_empty: Boolean,
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicQosOptions {
        #[serde(default)]
        pub global: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicConsumeOptions {
        #[serde(default)]
        pub no_local: Boolean,
        #[serde(default)]
        pub no_ack: Boolean,
        #[serde(default)]
        pub exclusive: Boolean,
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicCancelOptions {
        #[serde(default)]
        pub nowait: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicPublishOptions {
        #[serde(default)]
        pub mandatory: Boolean,
        #[serde(default)]
        pub immediate: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicDeliverOptions {
        #[serde(default)]
        pub redelivered: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicGetOptions {
        #[serde(default)]
        pub no_ack: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicGetOkOptions {
        #[serde(default)]
        pub redelivered: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicAckOptions {
        #[serde(default)]
        pub multiple: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicRejectOptions {
        #[serde(default)]
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicRecoverAsyncOptions {
        #[serde(default)]
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicRecoverOptions {
        #[serde(default)]
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct BasicNackOptions {
        #[serde(default)]
        pub multiple: Boolean,
        #[serde(default)]
        pub requeue: Boolean,
    }

    #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
    pub struct ConfirmSelectOptions {
        #[serde(default)]
        pub nowait: Boolean,
    }
}

use options::*;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum Reply {
    ConnectionOpenOk(PromiseResolver<()>, Connection),
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
    QueuePurgeOk(PromiseResolver<LongUInt>),
    QueueDeleteOk(PromiseResolver<LongUInt>, ShortString),
    QueueUnbindOk(
        PromiseResolver<()>,
        ShortString,
        ShortString,
        ShortString,
        FieldTable,
    ),
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
    BasicGetOk(
        PromiseResolver<Option<BasicGetMessage>>,
        ShortString,
        BasicGetOptions,
    ),
    BasicRecoverOk(PromiseResolver<()>),
    TxSelectOk(PromiseResolver<()>),
    TxCommitOk(PromiseResolver<()>),
    TxRollbackOk(PromiseResolver<()>),
    ConfirmSelectOk(PromiseResolver<()>),
}

impl Channel {
    pub(crate) fn receive_method(&self, method: AMQPClass) -> Result<()> {
        match method {
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
            AMQPClass::Tx(protocol::tx::AMQPMethod::SelectOk(m)) => self.receive_tx_select_ok(m),
            AMQPClass::Tx(protocol::tx::AMQPMethod::CommitOk(m)) => self.receive_tx_commit_ok(m),
            AMQPClass::Tx(protocol::tx::AMQPMethod::RollbackOk(m)) => {
                self.receive_tx_rollback_ok(m)
            }
            AMQPClass::Confirm(protocol::confirm::AMQPMethod::SelectOk(m)) => {
                self.receive_confirm_select_ok(m)
            }
            m => {
                error!("the client should not receive this method: {:?}", m);
                self.handle_invalid_contents(
                    format!("unexepcted method received on channel {}", self.id),
                    m.get_amqp_class_id(),
                    m.get_amqp_method_id(),
                )
            }
        }
    }

    fn receive_connection_start(&self, method: protocol::connection::Start) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_connection_start_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    fn connection_start_ok(
        &self,
        client_properties: FieldTable,
        mechanism: &str,
        response: &str,
        locale: &str,
        resolver: PromiseResolver<Connection>,
        connection: Connection,
        credentials: Credentials,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::StartOk(
            protocol::connection::StartOk {
                client_properties,
                mechanism: mechanism.into(),
                response: response.into(),
                locale: locale.into(),
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.start-ok".into());
        }
        self.send_method_frame(method, send_resolver, None);
        let end_hook_res = self.on_connection_start_ok_sent(resolver, connection, credentials);
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }

    fn receive_connection_secure(&self, method: protocol::connection::Secure) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_connection_secure_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    fn connection_secure_ok(&self, response: &str) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::SecureOk(
            protocol::connection::SecureOk {
                response: response.into(),
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.secure-ok".into());
        }
        self.send_method_frame(method, send_resolver, None);
        promise
    }

    fn receive_connection_tune(&self, method: protocol::connection::Tune) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_connection_tune_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    fn connection_tune_ok(
        &self,
        channel_max: ShortUInt,
        frame_max: LongUInt,
        heartbeat: ShortUInt,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::TuneOk(
            protocol::connection::TuneOk {
                channel_max,
                frame_max,
                heartbeat,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.tune-ok".into());
        }
        self.send_method_frame(method, send_resolver, None);
        promise
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connection_open(
        &self,
        virtual_host: &str,
        connection: Connection,
        conn_resolver: PromiseResolver<Connection>,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::Open(
            protocol::connection::Open {
                virtual_host: virtual_host.into(),
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.open".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("connection.open.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ConnectionOpenOk(resolver.clone(), connection),
                Box::new(resolver),
            )),
        );
        let end_hook_res = self.on_connection_open_sent(conn_resolver);
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }
    fn receive_connection_open_ok(&self, method: protocol::connection::OpenOk) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;

        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ConnectionOpenOk(resolver, connection)) => {
                let res = self.on_connection_open_ok_received(method, connection);
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted connection open-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connection_close(
        &self,
        reply_code: ShortUInt,
        reply_text: &str,
        class_id: ShortUInt,
        method_id: ShortUInt,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::Close(
            protocol::connection::Close {
                reply_code,
                reply_text: reply_text.into(),
                class_id,
                method_id,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.close".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("connection.close.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ConnectionCloseOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        let end_hook_res = self.on_connection_close_sent();
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }

    fn receive_connection_close(&self, method: protocol::connection::Close) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_connection_close_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connection_close_ok(&self, error: Error) -> Promise<()> {
        if !self.status.closing() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::CloseOk(
            protocol::connection::CloseOk {},
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.close-ok".into());
        }
        self.send_method_frame(method, send_resolver, None);
        let end_hook_res = self.on_connection_close_ok_sent(error);
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }
    fn receive_connection_close_ok(&self, method: protocol::connection::CloseOk) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;

        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ConnectionCloseOk(resolver)) => {
                let res = self.on_connection_close_ok_received();
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted connection close-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connection_blocked(&self, reason: &str) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::Blocked(
            protocol::connection::Blocked {
                reason: reason.into(),
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.blocked".into());
        }
        self.send_method_frame(method, send_resolver, None);
        promise
    }

    fn receive_connection_blocked(&self, method: protocol::connection::Blocked) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_connection_blocked_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connection_unblocked(&self) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::Unblocked(
            protocol::connection::Unblocked {},
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.unblocked".into());
        }
        self.send_method_frame(method, send_resolver, None);
        promise
    }

    fn receive_connection_unblocked(&self, method: protocol::connection::Unblocked) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_connection_unblocked_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connection_update_secret(&self, new_secret: &str, reason: &str) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Connection(protocol::connection::AMQPMethod::UpdateSecret(
            protocol::connection::UpdateSecret {
                new_secret: new_secret.into(),
                reason: reason.into(),
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("connection.update-secret".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("connection.update-secret.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ConnectionUpdateSecretOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_connection_update_secret_ok(
        &self,
        method: protocol::connection::UpdateSecretOk,
    ) -> Result<()> {
        self.assert_channel0(method.get_amqp_class_id(), method.get_amqp_method_id())?;

        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ConnectionUpdateSecretOk(resolver)) => {
                let res = Ok(());
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted connection update-secret-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn channel_open(&self, channel: Channel) -> PromiseChain<Channel> {
        if !self.status.initializing() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::Open(
            protocol::channel::Open {},
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("channel.open".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("channel.open.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ChannelOpenOk(resolver.clone(), channel),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_channel_open_ok(&self, method: protocol::channel::OpenOk) -> Result<()> {
        if !self.status.initializing() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ChannelOpenOk(resolver, channel)) => {
                self.on_channel_open_ok_received(method, resolver, channel)
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted channel open-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn channel_flow(&self, options: ChannelFlowOptions) -> PromiseChain<Boolean> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let ChannelFlowOptions { active } = options;
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::Flow(
            protocol::channel::Flow { active },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("channel.flow".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("channel.flow.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ChannelFlowOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }

    fn receive_channel_flow(&self, method: protocol::channel::Flow) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_channel_flow_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    fn channel_flow_ok(&self, options: ChannelFlowOkOptions) -> PromiseChain<()> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let ChannelFlowOkOptions { active } = options;
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::FlowOk(
            protocol::channel::FlowOk { active },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("channel.flow-ok".into());
        }
        self.send_method_frame(method, send_resolver, None);
        promise
    }
    fn receive_channel_flow_ok(&self, method: protocol::channel::FlowOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ChannelFlowOk(resolver)) => {
                self.on_channel_flow_ok_received(method, resolver)
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted channel flow-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn do_channel_close(
        &self,
        reply_code: ShortUInt,
        reply_text: &str,
        class_id: ShortUInt,
        method_id: ShortUInt,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        self.before_channel_close();
        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::Close(
            protocol::channel::Close {
                reply_code,
                reply_text: reply_text.into(),
                class_id,
                method_id,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("channel.close".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("channel.close.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ChannelCloseOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }

    fn receive_channel_close(&self, method: protocol::channel::Close) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_channel_close_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    fn channel_close_ok(&self, error: Error) -> Promise<()> {
        if !self.status.closing() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Channel(protocol::channel::AMQPMethod::CloseOk(
            protocol::channel::CloseOk {},
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("channel.close-ok".into());
        }
        self.send_method_frame(method, send_resolver, None);
        let end_hook_res = self.on_channel_close_ok_sent(error);
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }
    fn receive_channel_close_ok(&self, method: protocol::channel::CloseOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ChannelCloseOk(resolver)) => {
                let res = self.on_channel_close_ok_received();
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted channel close-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn access_request(&self, realm: &str, options: AccessRequestOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let AccessRequestOptions {
            exclusive,
            passive,
            active,
            write,
            read,
        } = options;
        let method = AMQPClass::Access(protocol::access::AMQPMethod::Request(
            protocol::access::Request {
                realm: realm.into(),

                exclusive,
                passive,
                active,
                write,
                read,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("access.request".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("access.request.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::AccessRequestOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_access_request_ok(&self, method: protocol::access::RequestOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::AccessRequestOk(resolver)) => {
                let res = self.on_access_request_ok_received(method);
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted access request-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn do_exchange_declare(
        &self,
        exchange: &str,
        kind: &str,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
        exchange_kind: ExchangeKind,
    ) -> PromiseChain<()> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let creation_arguments = arguments.clone();
        let ExchangeDeclareOptions {
            passive,
            durable,
            auto_delete,
            internal,
            nowait,
        } = options;
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Declare(
            protocol::exchange::Declare {
                exchange: exchange.into(),
                kind: kind.into(),

                passive,
                durable,
                auto_delete,
                internal,
                nowait,
                arguments,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("exchange.declare".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("exchange.declare.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ExchangeDeclareOk(
                    resolver.clone(),
                    exchange.into(),
                    exchange_kind,
                    options,
                    creation_arguments,
                ),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_exchange_declare_ok(protocol::exchange::DeclareOk {}) {
                return PromiseChain::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_exchange_declare_ok(&self, method: protocol::exchange::DeclareOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
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
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted exchange declare-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    /// Delete an exchange
    pub fn exchange_delete(&self, exchange: &str, options: ExchangeDeleteOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let ExchangeDeleteOptions { if_unused, nowait } = options;
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Delete(
            protocol::exchange::Delete {
                exchange: exchange.into(),

                if_unused,
                nowait,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("exchange.delete".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("exchange.delete.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ExchangeDeleteOk(resolver.clone(), exchange.into()),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_exchange_delete_ok(protocol::exchange::DeleteOk {}) {
                return Promise::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_exchange_delete_ok(&self, method: protocol::exchange::DeleteOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ExchangeDeleteOk(resolver, exchange)) => {
                let res = self.on_exchange_delete_ok_received(exchange);
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted exchange delete-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn exchange_bind(
        &self,
        destination: &str,
        source: &str,
        routing_key: &str,
        options: ExchangeBindOptions,
        arguments: FieldTable,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let creation_arguments = arguments.clone();
        let ExchangeBindOptions { nowait } = options;
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Bind(
            protocol::exchange::Bind {
                destination: destination.into(),
                source: source.into(),
                routing_key: routing_key.into(),

                nowait,
                arguments,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("exchange.bind".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("exchange.bind.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ExchangeBindOk(
                    resolver.clone(),
                    destination.into(),
                    source.into(),
                    routing_key.into(),
                    creation_arguments,
                ),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_exchange_bind_ok(protocol::exchange::BindOk {}) {
                return Promise::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_exchange_bind_ok(&self, method: protocol::exchange::BindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
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
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted exchange bind-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn exchange_unbind(
        &self,
        destination: &str,
        source: &str,
        routing_key: &str,
        options: ExchangeUnbindOptions,
        arguments: FieldTable,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let creation_arguments = arguments.clone();
        let ExchangeUnbindOptions { nowait } = options;
        let method = AMQPClass::Exchange(protocol::exchange::AMQPMethod::Unbind(
            protocol::exchange::Unbind {
                destination: destination.into(),
                source: source.into(),
                routing_key: routing_key.into(),

                nowait,
                arguments,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("exchange.unbind".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("exchange.unbind.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ExchangeUnbindOk(
                    resolver.clone(),
                    destination.into(),
                    source.into(),
                    routing_key.into(),
                    creation_arguments,
                ),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_exchange_unbind_ok(protocol::exchange::UnbindOk {}) {
                return Promise::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_exchange_unbind_ok(&self, method: protocol::exchange::UnbindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
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
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted exchange unbind-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn queue_declare(
        &self,
        queue: &str,
        options: QueueDeclareOptions,
        arguments: FieldTable,
    ) -> PromiseChain<Queue> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let creation_arguments = arguments.clone();
        let QueueDeclareOptions {
            passive,
            durable,
            exclusive,
            auto_delete,
            nowait,
        } = options;
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Declare(
            protocol::queue::Declare {
                queue: queue.into(),

                passive,
                durable,
                exclusive,
                auto_delete,
                nowait,
                arguments,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("queue.declare".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("queue.declare.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::QueueDeclareOk(resolver.clone(), options, creation_arguments),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_queue_declare_ok(protocol::queue::DeclareOk {
                queue: queue.into(),
                ..Default::default()
            }) {
                return PromiseChain::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_queue_declare_ok(&self, method: protocol::queue::DeclareOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::QueueDeclareOk(resolver, options, creation_arguments)) => {
                self.on_queue_declare_ok_received(method, resolver, options, creation_arguments)
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted queue declare-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn queue_bind(
        &self,
        queue: &str,
        exchange: &str,
        routing_key: &str,
        options: QueueBindOptions,
        arguments: FieldTable,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let creation_arguments = arguments.clone();
        let QueueBindOptions { nowait } = options;
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Bind(protocol::queue::Bind {
            queue: queue.into(),
            exchange: exchange.into(),
            routing_key: routing_key.into(),

            nowait,
            arguments,
        }));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("queue.bind".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("queue.bind.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::QueueBindOk(
                    resolver.clone(),
                    queue.into(),
                    exchange.into(),
                    routing_key.into(),
                    creation_arguments,
                ),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_queue_bind_ok(protocol::queue::BindOk {}) {
                return Promise::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_queue_bind_ok(&self, method: protocol::queue::BindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
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
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted queue bind-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn queue_purge(&self, queue: &str, options: QueuePurgeOptions) -> PromiseChain<LongUInt> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let QueuePurgeOptions { nowait } = options;
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Purge(protocol::queue::Purge {
            queue: queue.into(),

            nowait,
        }));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("queue.purge".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("queue.purge.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::QueuePurgeOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_queue_purge_ok(&self, method: protocol::queue::PurgeOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::QueuePurgeOk(resolver)) => {
                self.on_queue_purge_ok_received(method, resolver)
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted queue purge-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn queue_delete(&self, queue: &str, options: QueueDeleteOptions) -> PromiseChain<LongUInt> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let QueueDeleteOptions {
            if_unused,
            if_empty,
            nowait,
        } = options;
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Delete(
            protocol::queue::Delete {
                queue: queue.into(),

                if_unused,
                if_empty,
                nowait,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("queue.delete".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("queue.delete.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::QueueDeleteOk(resolver.clone(), queue.into()),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_queue_delete_ok(protocol::queue::DeleteOk {
                ..Default::default()
            }) {
                return PromiseChain::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_queue_delete_ok(&self, method: protocol::queue::DeleteOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::QueueDeleteOk(resolver, queue)) => {
                self.on_queue_delete_ok_received(method, resolver, queue)
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted queue delete-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn queue_unbind(
        &self,
        queue: &str,
        exchange: &str,
        routing_key: &str,
        arguments: FieldTable,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let creation_arguments = arguments.clone();
        let method = AMQPClass::Queue(protocol::queue::AMQPMethod::Unbind(
            protocol::queue::Unbind {
                queue: queue.into(),
                exchange: exchange.into(),
                routing_key: routing_key.into(),
                arguments,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("queue.unbind".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("queue.unbind.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::QueueUnbindOk(
                    resolver.clone(),
                    queue.into(),
                    exchange.into(),
                    routing_key.into(),
                    creation_arguments,
                ),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_queue_unbind_ok(&self, method: protocol::queue::UnbindOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
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
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted queue unbind-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_qos(&self, prefetch_count: ShortUInt, options: BasicQosOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let BasicQosOptions { global } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Qos(protocol::basic::Qos {
            prefetch_count,

            global,
        }));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.qos".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("basic.qos.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::BasicQosOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_basic_qos_ok(&self, method: protocol::basic::QosOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::BasicQosOk(resolver)) => {
                let res = Ok(());
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted basic qos-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn do_basic_consume(
        &self,
        queue: &str,
        consumer_tag: &str,
        options: BasicConsumeOptions,
        arguments: FieldTable,
        original: Option<Consumer>,
    ) -> PromiseChain<Consumer> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let creation_arguments = arguments.clone();
        let BasicConsumeOptions {
            no_local,
            no_ack,
            exclusive,
            nowait,
        } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Consume(
            protocol::basic::Consume {
                queue: queue.into(),
                consumer_tag: consumer_tag.into(),

                no_local,
                no_ack,
                exclusive,
                nowait,
                arguments,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.consume".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("basic.consume.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::BasicConsumeOk(
                    resolver.clone(),
                    self.channel_closer.clone(),
                    queue.into(),
                    options,
                    creation_arguments,
                    original,
                ),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_basic_consume_ok(protocol::basic::ConsumeOk {
                consumer_tag: consumer_tag.into(),
            }) {
                return PromiseChain::new_with_data(Err(err));
            }
        }
        promise
    }
    fn receive_basic_consume_ok(&self, method: protocol::basic::ConsumeOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
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
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted basic consume-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_cancel(&self, consumer_tag: &str, options: BasicCancelOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let BasicCancelOptions { nowait } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Cancel(
            protocol::basic::Cancel {
                consumer_tag: consumer_tag.into(),

                nowait,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.cancel".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("basic.cancel.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::BasicCancelOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        if nowait {
            if let Err(err) = self.receive_basic_cancel_ok(protocol::basic::CancelOk {
                consumer_tag: consumer_tag.into(),
            }) {
                return Promise::new_with_data(Err(err));
            }
        }
        promise
    }

    fn receive_basic_cancel(&self, method: protocol::basic::Cancel) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_basic_cancel_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    fn basic_cancel_ok(&self, consumer_tag: &str) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::CancelOk(
            protocol::basic::CancelOk {
                consumer_tag: consumer_tag.into(),
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.cancel-ok".into());
        }
        self.send_method_frame(method, send_resolver, None);
        promise
    }
    fn receive_basic_cancel_ok(&self, method: protocol::basic::CancelOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::BasicCancelOk(resolver)) => {
                let res = self.on_basic_cancel_ok_received(method);
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted basic cancel-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_publish(
        &self,
        exchange: &str,
        routing_key: &str,
        options: BasicPublishOptions,
        payload: Vec<u8>,
        properties: BasicProperties,
    ) -> PromiseChain<PublisherConfirm> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let start_hook_res = self.before_basic_publish();
        let BasicPublishOptions {
            mandatory,
            immediate,
        } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Publish(
            protocol::basic::Publish {
                exchange: exchange.into(),
                routing_key: routing_key.into(),

                mandatory,
                immediate,
            },
        ));

        self.send_method_frame_with_body(method, payload, properties, start_hook_res)
            .unwrap_or_else(|err| PromiseChain::new_with_data(Err(err)))
    }

    fn receive_basic_return(&self, method: protocol::basic::Return) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_basic_return_received(method)
    }

    fn receive_basic_deliver(&self, method: protocol::basic::Deliver) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_basic_deliver_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    fn do_basic_get(
        &self,
        queue: &str,
        options: BasicGetOptions,
        original: Option<PromiseResolver<Option<BasicGetMessage>>>,
    ) -> PromiseChain<Option<BasicGetMessage>> {
        if !self.status.connected() {
            return PromiseChain::new_with_data(Err(Error::InvalidChannelState(
                self.status.state(),
            )));
        }

        let BasicGetOptions { no_ack } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Get(protocol::basic::Get {
            queue: queue.into(),

            no_ack,
        }));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.get".into());
        }
        let (promise, resolver) = PromiseChain::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("basic.get.Ok".into());
        }
        let resolver = original.unwrap_or(resolver);
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::BasicGetOk(resolver.clone(), queue.into(), options),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_basic_get_ok(&self, method: protocol::basic::GetOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::BasicGetOk(resolver, queue, options)) => {
                self.on_basic_get_ok_received(method, resolver, queue, options)
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted basic get-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }

    fn receive_basic_get_empty(&self, method: protocol::basic::GetEmpty) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_basic_get_empty_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_ack(&self, delivery_tag: LongLongUInt, options: BasicAckOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let BasicAckOptions { multiple } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Ack(protocol::basic::Ack {
            delivery_tag,

            multiple,
        }));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.ack".into());
        }
        self.send_method_frame(method, send_resolver, None);
        let end_hook_res = self.on_basic_ack_sent(multiple, delivery_tag);
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }

    fn receive_basic_ack(&self, method: protocol::basic::Ack) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_basic_ack_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_reject(
        &self,
        delivery_tag: LongLongUInt,
        options: BasicRejectOptions,
    ) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let BasicRejectOptions { requeue } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Reject(
            protocol::basic::Reject {
                delivery_tag,

                requeue,
            },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.reject".into());
        }
        self.send_method_frame(method, send_resolver, None);
        promise
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_recover_async(&self, options: BasicRecoverAsyncOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let BasicRecoverAsyncOptions { requeue } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::RecoverAsync(
            protocol::basic::RecoverAsync { requeue },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.recover-async".into());
        }
        self.send_method_frame(method, send_resolver, None);
        let end_hook_res = self.on_basic_recover_async_sent();
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_recover(&self, options: BasicRecoverOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let BasicRecoverOptions { requeue } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Recover(
            protocol::basic::Recover { requeue },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.recover".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("basic.recover.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::BasicRecoverOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_basic_recover_ok(&self, method: protocol::basic::RecoverOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::BasicRecoverOk(resolver)) => {
                let res = self.on_basic_recover_ok_received();
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted basic recover-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn basic_nack(&self, delivery_tag: LongLongUInt, options: BasicNackOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let BasicNackOptions { multiple, requeue } = options;
        let method = AMQPClass::Basic(protocol::basic::AMQPMethod::Nack(protocol::basic::Nack {
            delivery_tag,

            multiple,
            requeue,
        }));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("basic.nack".into());
        }
        self.send_method_frame(method, send_resolver, None);
        let end_hook_res = self.on_basic_nack_sent(multiple, delivery_tag);
        if let Err(err) = end_hook_res {
            return Promise::new_with_data(Err(err));
        }
        promise
    }

    fn receive_basic_nack(&self, method: protocol::basic::Nack) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }
        self.on_basic_nack_received(method)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn tx_select(&self) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Tx(protocol::tx::AMQPMethod::Select(protocol::tx::Select {}));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("tx.select".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("tx.select.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::TxSelectOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_tx_select_ok(&self, method: protocol::tx::SelectOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::TxSelectOk(resolver)) => {
                let res = Ok(());
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted tx select-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn tx_commit(&self) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Tx(protocol::tx::AMQPMethod::Commit(protocol::tx::Commit {}));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("tx.commit".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("tx.commit.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::TxCommitOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_tx_commit_ok(&self, method: protocol::tx::CommitOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::TxCommitOk(resolver)) => {
                let res = Ok(());
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted tx commit-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn tx_rollback(&self) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let method = AMQPClass::Tx(protocol::tx::AMQPMethod::Rollback(
            protocol::tx::Rollback {},
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("tx.rollback".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("tx.rollback.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::TxRollbackOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_tx_rollback_ok(&self, method: protocol::tx::RollbackOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::TxRollbackOk(resolver)) => {
                let res = Ok(());
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!("unexepcted tx rollback-ok received on channel {}", self.id),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn confirm_select(&self, options: ConfirmSelectOptions) -> Promise<()> {
        if !self.status.connected() {
            return Promise::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
        }

        let ConfirmSelectOptions { nowait } = options;
        let method = AMQPClass::Confirm(protocol::confirm::AMQPMethod::Select(
            protocol::confirm::Select { nowait },
        ));

        let (promise, send_resolver) = Promise::new();
        if log_enabled!(Trace) {
            promise.set_marker("confirm.select".into());
        }
        let (promise, resolver) = Promise::after(promise);
        if log_enabled!(Trace) {
            promise.set_marker("confirm.select.Ok".into());
        }
        self.send_method_frame(
            method,
            send_resolver,
            Some(ExpectedReply(
                Reply::ConfirmSelectOk(resolver.clone()),
                Box::new(resolver),
            )),
        );
        promise
    }
    fn receive_confirm_select_ok(&self, method: protocol::confirm::SelectOk) -> Result<()> {
        if !self.status.can_receive_messages() {
            return Err(Error::InvalidChannelState(self.status.state()));
        }

        match self.frames.next_expected_reply(self.id) {
            Some(Reply::ConfirmSelectOk(resolver)) => {
                let res = self.on_confirm_select_ok_received();
                resolver.swear(res.clone());
                res
            }
            _ => self.handle_invalid_contents(
                format!(
                    "unexepcted confirm select-ok received on channel {}",
                    self.id
                ),
                method.get_amqp_class_id(),
                method.get_amqp_method_id(),
            ),
        }
    }
}
