use crate::{
    acknowledgement::{Acknowledgements, DeliveryTag},
    auth::Credentials,
    channel_status::{ChannelState, ChannelStatus},
    confirmation::Confirmation,
    connection::Connection,
    connection_status::ConnectionState,
    consumer::Consumer,
    executor::Executor,
    frames::{ExpectedReply, Priority},
    id_sequence::IdSequence,
    message::{BasicGetMessage, BasicReturnMessage, Delivery},
    protocol::{self, AMQPClass, AMQPError, AMQPSoftError},
    queue::Queue,
    queues::Queues,
    returned_messages::ReturnedMessages,
    types::*,
    wait::{Wait, WaitHandle},
    BasicProperties, Error, ExchangeKind, Result,
};
use amq_protocol::frame::{AMQPContentHeader, AMQPFrame};
use log::{debug, error, info, trace};
use std::{borrow::Borrow, sync::Arc};

#[cfg(test)]
use crate::queue::QueueState;

#[derive(Clone, Debug)]
pub struct Channel {
    id: u16,
    connection: Connection,
    status: ChannelStatus,
    acknowledgements: Acknowledgements,
    delivery_tag: IdSequence<DeliveryTag>,
    queues: Queues,
    returned_messages: ReturnedMessages,
    executor: Arc<dyn Executor>,
}

impl Channel {
    pub(crate) fn new(
        channel_id: u16,
        connection: Connection,
        executor: Arc<dyn Executor>,
    ) -> Channel {
        let returned_messages = ReturnedMessages::default();
        Channel {
            id: channel_id,
            connection,
            status: ChannelStatus::default(),
            acknowledgements: Acknowledgements::new(returned_messages.clone()),
            delivery_tag: IdSequence::new(false),
            queues: Queues::default(),
            returned_messages,
            executor,
        }
    }

    pub fn status(&self) -> &ChannelStatus {
        &self.status
    }

    fn set_closed(&self) -> Result<()> {
        self.set_state(ChannelState::Closed);
        self.cancel_consumers()
            .and(self.connection.remove_channel(self.id))
    }

    fn set_error(&self) -> Result<()> {
        self.set_state(ChannelState::Error);
        self.error_consumers()
            .and(self.connection.remove_channel(self.id))
    }

    pub(crate) fn cancel_consumers(&self) -> Result<()> {
        self.queues.cancel_consumers()
    }

    pub(crate) fn error_consumers(&self) -> Result<()> {
        self.queues.error_consumers()
    }

    pub(crate) fn set_state(&self, state: ChannelState) {
        self.status.set_state(state);
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    pub fn close(&self, reply_code: ShortUInt, reply_text: &str) -> Confirmation<()> {
        self.do_channel_close(reply_code, reply_text, 0, 0)
    }

    pub fn basic_consume(
        &self,
        queue: &Queue,
        consumer_tag: &str,
        options: BasicConsumeOptions,
        arguments: FieldTable,
    ) -> Confirmation<Consumer> {
        self.do_basic_consume(queue.borrow(), consumer_tag, options, arguments)
    }

    pub fn exchange_declare(
        &self,
        exchange: &str,
        kind: ExchangeKind,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
    ) -> Confirmation<()> {
        self.do_exchange_declare(exchange, kind.kind(), options, arguments)
    }

    pub fn wait_for_confirms(&self) -> Confirmation<Vec<BasicReturnMessage>> {
        if let Some(wait) = self.acknowledgements.get_last_pending() {
            trace!("Waiting for pending confirms");
            let returned_messages = self.returned_messages.clone();
            Confirmation::new(wait).map(Box::new(move |_| returned_messages.drain()))
        } else {
            trace!("No confirms to wait for");
            let (wait, wait_handle) = Wait::new();
            wait_handle.finish(Vec::default());
            Confirmation::new(wait)
        }
    }

    #[cfg(test)]
    pub(crate) fn register_queue(&self, queue: QueueState) {
        self.queues.register(queue);
    }

    pub(crate) fn send_method_frame(
        &self,
        method: AMQPClass,
        expected_reply: Option<ExpectedReply>,
    ) -> Result<Wait<()>> {
        self.send_frame(
            Priority::NORMAL,
            AMQPFrame::Method(self.id, method),
            expected_reply,
        )
    }

    fn send_method_frame_with_body(
        &self,
        method: AMQPClass,
        payload: Vec<u8>,
        properties: BasicProperties,
    ) -> Result<Wait<()>> {
        let class_id = method.get_amqp_class_id();
        let header = AMQPContentHeader {
            class_id,
            weight: 0,
            body_size: payload.len() as u64,
            properties,
        };
        let frame_max = self.connection.configuration().frame_max();
        let mut frames = vec![(
            AMQPFrame::Method(self.id, method),
            Some(AMQPFrame::Header(self.id, class_id, Box::new(header))),
        )];

        // a content body frame 8 bytes of overhead
        frames.extend(
            payload
                .as_slice()
                .chunks(frame_max as usize - 8)
                .map(|chunk| (AMQPFrame::Body(self.id, chunk.into()), None)),
        );

        self.connection.send_frames(self.id, frames)
    }

    pub(crate) fn send_frame(
        &self,
        priority: Priority,
        frame: AMQPFrame,
        expected_reply: Option<ExpectedReply>,
    ) -> Result<Wait<()>> {
        self.connection
            .send_frame(self.id, priority, frame, expected_reply)
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        size: u64,
        properties: BasicProperties,
    ) -> Result<()> {
        if let ChannelState::WillReceiveContent(queue_name, request_id_or_consumer_tag) =
            self.status.state()
        {
            if size > 0 {
                self.status.set_state(ChannelState::ReceivingContent(
                    queue_name.clone(),
                    request_id_or_consumer_tag.clone(),
                    size as usize,
                ));
            } else {
                self.status.set_state(ChannelState::Connected);
            }
            if let Some(queue_name) = queue_name {
                self.queues.handle_content_header_frame(
                    queue_name.as_str(),
                    request_id_or_consumer_tag,
                    size,
                    properties,
                )?;
            } else {
                self.returned_messages.set_delivery_properties(properties);
                if size == 0 {
                    self.returned_messages.new_delivery_complete();
                }
            }
            Ok(())
        } else {
            self.set_error()
        }
    }

    pub(crate) fn handle_body_frame(&self, payload: Vec<u8>) -> Result<()> {
        let payload_size = payload.len();

        if let ChannelState::ReceivingContent(
            queue_name,
            request_id_or_consumer_tag,
            remaining_size,
        ) = self.status.state()
        {
            if remaining_size >= payload_size {
                if let Some(queue_name) = queue_name.as_ref() {
                    self.queues.handle_body_frame(
                        queue_name.as_str(),
                        request_id_or_consumer_tag.clone(),
                        remaining_size,
                        payload_size,
                        payload,
                    )?;
                } else {
                    self.returned_messages.receive_delivery_content(payload);
                    if remaining_size == payload_size {
                        self.returned_messages.new_delivery_complete();
                    }
                }
                if remaining_size == payload_size {
                    self.status.set_state(ChannelState::Connected);
                } else {
                    self.status.set_state(ChannelState::ReceivingContent(
                        queue_name,
                        request_id_or_consumer_tag,
                        remaining_size - payload_size,
                    ));
                }
                Ok(())
            } else {
                error!("body frame too large");
                self.set_error()
            }
        } else {
            self.set_error()
        }
    }

    fn before_basic_publish(&self) {
        if self.status.confirm() {
            let delivery_tag = self.delivery_tag.next();
            self.acknowledgements.register_pending(delivery_tag);
        }
    }

    fn acknowledgement_error(&self, error: Error, class_id: u16, method_id: u16) -> Result<()> {
        self.do_channel_close(
            AMQPSoftError::PRECONDITIONFAILED.get_id(),
            "precondition failed",
            class_id,
            method_id,
        )
        .into_error()?;
        Err(error)
    }

    fn on_connection_start_ok_sent(
        &self,
        wait_handle: WaitHandle<Connection>,
        credentials: Credentials,
    ) -> Result<()> {
        self.connection
            .set_state(ConnectionState::SentStartOk(wait_handle, credentials));
        Ok(())
    }

    fn on_connection_open_sent(&self, wait_handle: WaitHandle<Connection>) -> Result<()> {
        self.connection
            .set_state(ConnectionState::SentOpen(wait_handle));
        Ok(())
    }

    fn on_connection_close_sent(&self) -> Result<()> {
        self.connection.set_closing();
        Ok(())
    }

    fn on_connection_close_ok_sent(&self) -> Result<()> {
        self.connection.set_closed()
    }

    fn on_channel_close_sent(&self) -> Result<()> {
        self.set_state(ChannelState::Closing);
        Ok(())
    }

    fn on_channel_close_ok_sent(&self) -> Result<()> {
        self.set_closed()
    }

    fn on_basic_recover_async_sent(&self) -> Result<()> {
        self.queues.drop_prefetched_messages()
    }

    fn on_basic_ack_sent(&self, multiple: bool, delivery_tag: DeliveryTag) -> Result<()> {
        if multiple && delivery_tag == 0 {
            self.queues.drop_prefetched_messages()
        } else {
            Ok(())
        }
    }

    fn on_basic_nack_sent(&self, multiple: bool, delivery_tag: DeliveryTag) -> Result<()> {
        if multiple && delivery_tag == 0 {
            self.queues.drop_prefetched_messages()
        } else {
            Ok(())
        }
    }

    fn tune_connection_configuration(&self, channel_max: u16, frame_max: u32, heartbeat: u16) {
        // If we disable the heartbeat (0) but the server don't, follow him and enable it too
        // If both us and the server want heartbeat enabled, pick the lowest value.
        if self.connection.configuration().heartbeat() == 0
            || heartbeat != 0 && heartbeat < self.connection.configuration().heartbeat()
        {
            self.connection.configuration().set_heartbeat(heartbeat);
        }

        if channel_max != 0 {
            // 0 means we want to take the server's value
            // If both us and the server specified a channel_max, pick the lowest value.
            if self.connection.configuration().channel_max() == 0
                || channel_max < self.connection.configuration().channel_max()
            {
                self.connection.configuration().set_channel_max(channel_max);
            }
        }
        if self.connection.configuration().channel_max() == 0 {
            self.connection
                .configuration()
                .set_channel_max(u16::max_value());
        }

        if frame_max != 0 {
            // 0 means we want to take the server's value
            // If both us and the server specified a frame_max, pick the lowest value.
            if self.connection.configuration().frame_max() == 0
                || frame_max < self.connection.configuration().frame_max()
            {
                self.connection.configuration().set_frame_max(frame_max);
            }
        }
        if self.connection.configuration().frame_max() == 0 {
            self.connection
                .configuration()
                .set_frame_max(u32::max_value());
        }
    }

    fn on_connection_start_received(&self, method: protocol::connection::Start) -> Result<()> {
        trace!("Server sent connection::Start: {:?}", method);
        let state = self.connection.status().state();
        if let ConnectionState::SentProtocolHeader(wait_handle, credentials, mut options) = state {
            let mechanism = options.mechanism.to_string();
            let locale = options.locale.clone();

            if !method.mechanisms.split_whitespace().any(|m| m == mechanism) {
                error!("unsupported mechanism: {}", mechanism);
            }
            if !method.locales.split_whitespace().any(|l| l == locale) {
                error!("unsupported locale: {}", mechanism);
            }

            if !options.client_properties.contains_key("product")
                || !options.client_properties.contains_key("version")
            {
                options.client_properties.insert(
                    "product".into(),
                    AMQPValue::LongString(env!("CARGO_PKG_NAME").into()),
                );
                options.client_properties.insert(
                    "version".into(),
                    AMQPValue::LongString(env!("CARGO_PKG_VERSION").into()),
                );
            }

            options
                .client_properties
                .insert("platform".into(), AMQPValue::LongString("rust".into()));

            let mut capabilities = FieldTable::default();
            capabilities.insert("publisher_confirms".into(), AMQPValue::Boolean(true));
            capabilities.insert(
                "exchange_exchange_bindings".into(),
                AMQPValue::Boolean(true),
            );
            capabilities.insert("basic.nack".into(), AMQPValue::Boolean(true));
            capabilities.insert("consumer_cancel_notify".into(), AMQPValue::Boolean(true));
            capabilities.insert("connection.blocked".into(), AMQPValue::Boolean(true));
            capabilities.insert(
                "authentication_failure_close".into(),
                AMQPValue::Boolean(true),
            );

            options
                .client_properties
                .insert("capabilities".into(), AMQPValue::FieldTable(capabilities));

            self.connection_start_ok(
                options.client_properties,
                &mechanism,
                &credentials.sasl_auth_string(options.mechanism),
                &locale,
                wait_handle,
                credentials,
            )
            .into_error()
        } else {
            error!("Invalid state: {:?}", state);
            self.connection.set_error()?;
            Err(Error::InvalidConnectionState(state))
        }
    }

    fn on_connection_secure_received(&self, method: protocol::connection::Secure) -> Result<()> {
        trace!("Server sent connection::Secure: {:?}", method);

        let state = self.connection.status().state();
        if let ConnectionState::SentStartOk(_, credentials) = state {
            self.connection_secure_ok(&credentials.rabbit_cr_demo_answer())
                .into_error()
        } else {
            error!("Invalid state: {:?}", state);
            self.connection.set_error()?;
            Err(Error::InvalidConnectionState(state))
        }
    }

    fn on_connection_tune_received(&self, method: protocol::connection::Tune) -> Result<()> {
        debug!("Server sent Connection::Tune: {:?}", method);

        let state = self.connection.status().state();
        if let ConnectionState::SentStartOk(wait_handle, _) = state {
            self.tune_connection_configuration(
                method.channel_max,
                method.frame_max,
                method.heartbeat,
            );

            self.connection_tune_ok(
                self.connection.configuration().channel_max(),
                self.connection.configuration().frame_max(),
                self.connection.configuration().heartbeat(),
            )
            .into_error()?;
            self.connection_open(&self.connection.status().vhost(), wait_handle)
                .into_error()
        } else {
            error!("Invalid state: {:?}", state);
            self.connection.set_error()?;
            Err(Error::InvalidConnectionState(state))
        }
    }

    fn on_connection_open_ok_received(&self, _: protocol::connection::OpenOk) -> Result<()> {
        let state = self.connection.status().state();
        if let ConnectionState::SentOpen(wait_handle) = state {
            self.connection.set_state(ConnectionState::Connected);
            wait_handle.finish(self.connection.clone());
            Ok(())
        } else {
            error!("Invalid state: {:?}", state);
            self.connection.set_error()?;
            Err(Error::InvalidConnectionState(state))
        }
    }

    fn on_connection_close_received(&self, method: protocol::connection::Close) -> Result<()> {
        if let Some(error) = AMQPError::from_id(method.reply_code) {
            error!(
                "Connection closed on channel {} by {}:{} => {:?} => {}",
                self.id, method.class_id, method.method_id, error, method.reply_text
            );
        } else {
            info!("Connection closed on channel {}: {:?}", self.id, method);
        }
        let state = self.connection.status().state();
        self.connection.set_closing();
        self.connection.drop_pending_frames();
        match state {
            ConnectionState::SentProtocolHeader(wait_handle, ..) => {
                wait_handle.error(Error::ConnectionRefused)
            }
            ConnectionState::SentStartOk(wait_handle, _) => {
                wait_handle.error(Error::ConnectionRefused)
            }
            ConnectionState::SentOpen(wait_handle) => wait_handle.error(Error::ConnectionRefused),
            _ => {}
        }
        self.connection_close_ok().into_error()?;
        Ok(())
    }

    fn on_connection_blocked_received(&self, _method: protocol::connection::Blocked) -> Result<()> {
        self.connection.do_block();
        Ok(())
    }

    fn on_connection_unblocked_received(
        &self,
        _method: protocol::connection::Unblocked,
    ) -> Result<()> {
        self.connection.do_unblock()
    }

    fn on_connection_close_ok_received(&self) -> Result<()> {
        self.connection.set_closed()
    }

    fn on_channel_open_ok_received(
        &self,
        _method: protocol::channel::OpenOk,
        wait_handle: WaitHandle<Channel>,
    ) -> Result<()> {
        self.status.set_state(ChannelState::Connected);
        wait_handle.finish(self.clone());
        Ok(())
    }

    fn on_channel_flow_received(&self, method: protocol::channel::Flow) -> Result<()> {
        self.status.set_send_flow(method.active);
        self.channel_flow_ok(ChannelFlowOkOptions {
            active: method.active,
        })
        .into_error()
    }

    fn on_channel_flow_ok_received(
        &self,
        method: protocol::channel::FlowOk,
        wait_handle: WaitHandle<Boolean>,
    ) -> Result<()> {
        // Nothing to do here, the server just confirmed that we paused/resumed the receiving flow
        wait_handle.finish(method.active);
        Ok(())
    }

    fn on_channel_close_received(&self, method: protocol::channel::Close) -> Result<()> {
        if let Some(error) = AMQPError::from_id(method.reply_code) {
            error!(
                "Channel {} closed by {}:{} => {:?} => {}",
                self.id, method.class_id, method.method_id, error, method.reply_text
            );
        } else {
            info!("Channel {} closed: {:?}", self.id, method);
        }
        self.channel_close_ok().into_error()
    }

    fn on_channel_close_ok_received(&self) -> Result<()> {
        self.set_closed()
    }

    fn on_queue_delete_ok_received(
        &self,
        method: protocol::queue::DeleteOk,
        wait_handle: WaitHandle<LongUInt>,
        queue: ShortString,
    ) -> Result<()> {
        self.queues.deregister(queue.as_str());
        wait_handle.finish(method.message_count);
        Ok(())
    }

    fn on_queue_purge_ok_received(
        &self,
        method: protocol::queue::PurgeOk,
        wait_handle: WaitHandle<LongUInt>,
    ) -> Result<()> {
        wait_handle.finish(method.message_count);
        Ok(())
    }

    fn on_queue_declare_ok_received(
        &self,
        method: protocol::queue::DeclareOk,
        wait_handle: WaitHandle<Queue>,
    ) -> Result<()> {
        let queue = Queue::new(method.queue, method.message_count, method.consumer_count);
        wait_handle.finish(queue.clone());
        self.queues.register(queue.into());
        Ok(())
    }

    fn on_basic_get_ok_received(
        &self,
        method: protocol::basic::GetOk,
        wait_handle: WaitHandle<Option<BasicGetMessage>>,
        queue: ShortString,
    ) -> Result<()> {
        self.queues.start_basic_get_delivery(
            queue.as_str(),
            BasicGetMessage::new(
                method.delivery_tag,
                method.exchange,
                method.routing_key,
                method.redelivered,
                method.message_count,
            ),
            wait_handle,
        );
        self.status
            .set_state(ChannelState::WillReceiveContent(Some(queue), None));
        Ok(())
    }

    fn on_basic_get_empty_received(&self, _: protocol::basic::GetEmpty) -> Result<()> {
        match self.connection.next_expected_reply(self.id) {
            Some(Reply::BasicGetOk(wait_handle, _)) => {
                wait_handle.finish(None);
                Ok(())
            }
            _ => {
                self.set_error()?;
                Err(Error::UnexpectedReply)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_basic_consume_ok_received(
        &self,
        method: protocol::basic::ConsumeOk,
        wait_handle: WaitHandle<Consumer>,
        queue: ShortString,
    ) -> Result<()> {
        let consumer = Consumer::new(method.consumer_tag.clone(), self.executor.clone());
        self.queues
            .register_consumer(queue.as_str(), method.consumer_tag, consumer.clone());
        wait_handle.finish(consumer);
        Ok(())
    }

    fn on_basic_deliver_received(&self, method: protocol::basic::Deliver) -> Result<()> {
        if let Some(queue_name) = self.queues.start_consumer_delivery(
            method.consumer_tag.as_str(),
            Delivery::new(
                method.delivery_tag,
                method.exchange,
                method.routing_key,
                method.redelivered,
            ),
        ) {
            self.status.set_state(ChannelState::WillReceiveContent(
                Some(queue_name),
                Some(method.consumer_tag),
            ));
        }
        Ok(())
    }

    fn on_basic_cancel_received(&self, method: protocol::basic::Cancel) -> Result<()> {
        self.queues
            .deregister_consumer(method.consumer_tag.as_str())
            .and(if !method.nowait {
                self.basic_cancel_ok(method.consumer_tag.as_str())
                    .into_error()
            } else {
                Ok(())
            })
    }

    fn on_basic_cancel_ok_received(&self, method: protocol::basic::CancelOk) -> Result<()> {
        self.queues
            .deregister_consumer(method.consumer_tag.as_str())
    }

    fn on_basic_ack_received(&self, method: protocol::basic::Ack) -> Result<()> {
        if self.status.confirm() {
            if method.multiple {
                if method.delivery_tag > 0 {
                    self.acknowledgements
                        .ack_all_before(method.delivery_tag)
                        .or_else(|err| {
                            self.acknowledgement_error(
                                err,
                                method.get_amqp_class_id(),
                                method.get_amqp_method_id(),
                            )
                        })?;
                } else {
                    self.acknowledgements.ack_all_pending();
                }
            } else {
                self.acknowledgements
                    .ack(method.delivery_tag)
                    .or_else(|err| {
                        self.acknowledgement_error(
                            err,
                            method.get_amqp_class_id(),
                            method.get_amqp_method_id(),
                        )
                    })?;
            }
        }
        Ok(())
    }

    fn on_basic_nack_received(&self, method: protocol::basic::Nack) -> Result<()> {
        if self.status.confirm() {
            if method.multiple {
                if method.delivery_tag > 0 {
                    self.acknowledgements
                        .nack_all_before(method.delivery_tag)
                        .or_else(|err| {
                            self.acknowledgement_error(
                                err,
                                method.get_amqp_class_id(),
                                method.get_amqp_method_id(),
                            )
                        })?;
                } else {
                    self.acknowledgements.nack_all_pending();
                }
            } else {
                self.acknowledgements
                    .nack(method.delivery_tag)
                    .or_else(|err| {
                        self.acknowledgement_error(
                            err,
                            method.get_amqp_class_id(),
                            method.get_amqp_method_id(),
                        )
                    })?;
            }
        }
        Ok(())
    }

    fn on_basic_return_received(&self, method: protocol::basic::Return) -> Result<()> {
        self.returned_messages
            .start_new_delivery(BasicReturnMessage::new(
                method.exchange,
                method.routing_key,
                method.reply_code,
                method.reply_text,
            ));
        self.status
            .set_state(ChannelState::WillReceiveContent(None, None));
        Ok(())
    }

    fn on_basic_recover_ok_received(&self) -> Result<()> {
        self.queues.drop_prefetched_messages()
    }

    fn on_confirm_select_ok_received(&self) -> Result<()> {
        self.status.set_confirm();
        Ok(())
    }

    fn on_access_request_ok_received(&self, _: protocol::access::RequestOk) -> Result<()> {
        Ok(())
    }
}

include!(concat!(env!("OUT_DIR"), "/channel.rs"));
