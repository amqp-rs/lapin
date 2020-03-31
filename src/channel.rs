use crate::{
    acknowledgement::{Acknowledgements, DeliveryTag},
    auth::Credentials,
    channel_status::{ChannelState, ChannelStatus},
    connection::Connection,
    connection_status::ConnectionState,
    consumer::Consumer,
    executor::Executor,
    frames::{ExpectedReply, Priority},
    id_sequence::IdSequence,
    message::{BasicGetMessage, BasicReturnMessage, Delivery},
    pinky_swear::{Pinky, PinkySwear},
    protocol::{self, AMQPClass, AMQPError, AMQPSoftError},
    queue::Queue,
    queues::Queues,
    returned_messages::ReturnedMessages,
    types::*,
    BasicProperties, Error, ExchangeKind, PublisherConfirm, Result,
};
use amq_protocol::frame::{AMQPContentHeader, AMQPFrame};
use log::{debug, error, info, trace};
use parking_lot::Mutex;
use std::{convert::TryFrom, sync::Arc};

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

    fn set_closed(&self, error: Error) -> Result<()> {
        self.set_state(ChannelState::Closed);
        self.cancel_consumers()
            .and(self.connection.remove_channel(self.id, error))
    }

    fn set_error(&self, error: Error) -> Result<()> {
        self.set_state(ChannelState::Error);
        self.error_consumers(error.clone())
            .and(self.connection.remove_channel(self.id, error))
    }

    pub(crate) fn cancel_consumers(&self) -> Result<()> {
        self.queues.cancel_consumers()
    }

    pub(crate) fn error_consumers(&self, error: Error) -> Result<()> {
        self.queues.error_consumers(error)
    }

    pub(crate) fn set_state(&self, state: ChannelState) {
        self.status.set_state(state);
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    pub fn close(&self, reply_code: ShortUInt, reply_text: &str) -> PinkySwear<Result<()>> {
        self.do_channel_close(reply_code, reply_text, 0, 0)
    }

    pub fn exchange_declare(
        &self,
        exchange: &str,
        kind: ExchangeKind,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
    ) -> PinkySwear<Result<()>> {
        self.do_exchange_declare(exchange, kind.kind(), options, arguments)
    }

    pub fn wait_for_confirms(
        &self,
    ) -> PinkySwear<Result<Vec<BasicReturnMessage>>, Result<PublisherConfirm>> {
        if let Some(promise) = self.acknowledgements.get_last_pending() {
            trace!("Waiting for pending confirms");
            let returned_messages = self.returned_messages.clone();
            promise.traverse(Box::new(move |_| Ok(returned_messages.drain())))
        } else {
            trace!("No confirms to wait for");
            PinkySwear::new_with_data(Ok(Vec::default()))
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
    ) -> Result<PinkySwear<Result<()>>> {
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
        publisher_confirms_result: Option<PinkySwear<Result<PublisherConfirm>>>,
    ) -> Result<PinkySwear<Result<PinkySwear<Result<PublisherConfirm>>>, Result<()>>> {
        let class_id = method.get_amqp_class_id();
        let header = AMQPContentHeader {
            class_id,
            weight: 0,
            body_size: payload.len() as u64,
            properties,
        };
        let frame_max = self.connection.configuration().frame_max();
        let mut frames = vec![
            AMQPFrame::Method(self.id, method),
            AMQPFrame::Header(self.id, class_id, Box::new(header)),
        ];

        // a content body frame 8 bytes of overhead
        frames.extend(
            payload
                .as_slice()
                .chunks(frame_max as usize - 8)
                .map(|chunk| AMQPFrame::Body(self.id, chunk.into())),
        );

        // tweak to make rustc happy
        let publisher_confirms_result = Arc::new(Mutex::new(publisher_confirms_result));

        Ok(self
            .connection
            .send_frames(self.id, frames)?
            .traverse(Box::new(move |res| {
                res.map(|()| publisher_confirms_result.lock().take().unwrap_or_else(|| PinkySwear::new_with_data(Ok(PublisherConfirm::NotRequested))))
            })))
    }

    pub(crate) fn send_frame(
        &self,
        priority: Priority,
        frame: AMQPFrame,
        expected_reply: Option<ExpectedReply>,
    ) -> Result<PinkySwear<Result<()>>> {
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
                self.set_state(ChannelState::ReceivingContent(
                    queue_name.clone(),
                    request_id_or_consumer_tag.clone(),
                    size as usize,
                ));
            } else {
                self.set_state(ChannelState::Connected);
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
            self.set_error(Error::InvalidChannelState(self.status.state()))
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
                    self.set_state(ChannelState::Connected);
                } else {
                    self.set_state(ChannelState::ReceivingContent(
                        queue_name,
                        request_id_or_consumer_tag,
                        remaining_size - payload_size,
                    ));
                }
                Ok(())
            } else {
                error!("body frame too large");
                self.set_error(Error::InvalidBodyReceived)
            }
        } else {
            self.set_error(Error::InvalidChannelState(self.status.state()))
        }
    }

    fn before_basic_publish(&self) -> Option<PinkySwear<Result<PublisherConfirm>>> {
        if self.status.confirm() {
            let delivery_tag = self.delivery_tag.next();
            Some(self.acknowledgements.register_pending(delivery_tag))
        } else {
            None
        }
    }

    fn acknowledgement_error(&self, error: Error, class_id: u16, method_id: u16) -> Result<()> {
        self.do_channel_close(
            AMQPSoftError::PRECONDITIONFAILED.get_id(),
            "precondition failed",
            class_id,
            method_id,
        )
        .try_wait()
        .transpose()
        .map(|_| ())?;
        Err(error)
    }

    fn on_connection_start_ok_sent(
        &self,
        pinky: Pinky<Result<Connection>, Result<()>>,
        credentials: Credentials,
    ) -> Result<()> {
        self.connection
            .set_state(ConnectionState::SentStartOk(pinky, credentials));
        Ok(())
    }

    fn on_connection_open_sent(&self, pinky: Pinky<Result<Connection>, Result<()>>) -> Result<()> {
        self.connection.set_state(ConnectionState::SentOpen(pinky));
        Ok(())
    }

    fn on_connection_close_sent(&self) -> Result<()> {
        self.connection.set_closing();
        Ok(())
    }

    fn on_connection_close_ok_sent(&self) -> Result<()> {
        self.connection.set_closed(Error::InvalidConnectionState(
            self.connection.status().state(),
        ))
    }

    fn before_channel_close(&self) {
        self.set_state(ChannelState::Closing);
    }

    fn on_channel_close_ok_sent(&self, error: Error) -> Result<()> {
        self.set_closed(error)
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
        if let ConnectionState::SentProtocolHeader(pinky, credentials, mut options) = state {
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
                pinky,
                credentials,
            )
            .try_wait()
            .transpose()
            .map(|_| ())
        } else {
            error!("Invalid state: {:?}", state);
            let error = Error::InvalidConnectionState(state);
            self.connection.set_error(error.clone())?;
            Err(error)
        }
    }

    fn on_connection_secure_received(&self, method: protocol::connection::Secure) -> Result<()> {
        trace!("Server sent connection::Secure: {:?}", method);

        let state = self.connection.status().state();
        if let ConnectionState::SentStartOk(_, credentials) = state {
            self.connection_secure_ok(&credentials.rabbit_cr_demo_answer())
                .try_wait()
                .transpose()
                .map(|_| ())
        } else {
            error!("Invalid state: {:?}", state);
            let error = Error::InvalidConnectionState(state);
            self.connection.set_error(error.clone())?;
            Err(error)
        }
    }

    fn on_connection_tune_received(&self, method: protocol::connection::Tune) -> Result<()> {
        debug!("Server sent Connection::Tune: {:?}", method);

        let state = self.connection.status().state();
        if let ConnectionState::SentStartOk(pinky, _) = state {
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
            .try_wait()
            .transpose()
            .map(|_| ())?;
            self.connection_open(&self.connection.status().vhost(), pinky)
                .try_wait()
                .transpose()
                .map(|_| ())
        } else {
            error!("Invalid state: {:?}", state);
            let error = Error::InvalidConnectionState(state);
            self.connection.set_error(error.clone())?;
            Err(error)
        }
    }

    fn on_connection_open_ok_received(&self, _: protocol::connection::OpenOk) -> Result<()> {
        let state = self.connection.status().state();
        if let ConnectionState::SentOpen(pinky) = state {
            self.connection.set_state(ConnectionState::Connected);
            pinky.swear(Ok(self.connection.clone()));
            Ok(())
        } else {
            error!("Invalid state: {:?}", state);
            let error = Error::InvalidConnectionState(state);
            self.connection.set_error(error.clone())?;
            Err(error)
        }
    }

    fn on_connection_close_received(&self, method: protocol::connection::Close) -> Result<()> {
        let error = AMQPError::try_from(method.clone())
            .map(|error| {
                error!(
                    "Connection closed on channel {} by {}:{} => {:?} => {}",
                    self.id, method.class_id, method.method_id, error, method.reply_text
                );
                Error::ProtocolError(error)
            })
            .unwrap_or_else(|error| {
                error!("{}", error);
                info!("Connection closed on channel {}: {:?}", self.id, method);
                Error::InvalidConnectionState(ConnectionState::Closed)
            });
        let state = self.connection.status().state();
        self.connection.set_closing();
        self.connection.drop_pending_frames(error.clone());
        match state {
            ConnectionState::SentProtocolHeader(pinky, ..) => pinky.swear(Err(error)),
            ConnectionState::SentStartOk(pinky, _) => pinky.swear(Err(error)),
            ConnectionState::SentOpen(pinky) => pinky.swear(Err(error)),
            _ => {}
        }
        self.connection_close_ok()
            .try_wait()
            .transpose()
            .map(|_| ())?;
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
        self.connection
            .set_closed(Error::InvalidConnectionState(ConnectionState::Closed))
    }

    fn on_channel_open_ok_received(
        &self,
        _method: protocol::channel::OpenOk,
        pinky: Pinky<Result<Channel>>,
    ) -> Result<()> {
        self.set_state(ChannelState::Connected);
        pinky.swear(Ok(self.clone()));
        Ok(())
    }

    fn on_channel_flow_received(&self, method: protocol::channel::Flow) -> Result<()> {
        self.status.set_send_flow(method.active);
        self.channel_flow_ok(ChannelFlowOkOptions {
            active: method.active,
        })
        .try_wait()
        .transpose()
        .map(|_| ())
    }

    fn on_channel_flow_ok_received(
        &self,
        method: protocol::channel::FlowOk,
        pinky: Pinky<Result<Boolean>>,
    ) -> Result<()> {
        // Nothing to do here, the server just confirmed that we paused/resumed the receiving flow
        pinky.swear(Ok(method.active));
        Ok(())
    }

    fn on_channel_close_received(&self, method: protocol::channel::Close) -> Result<()> {
        let error = AMQPError::try_from(method.clone())
            .map(|error| {
                error!(
                    "Channel closed on channel {} by {}:{} => {:?} => {}",
                    self.id, method.class_id, method.method_id, error, method.reply_text
                );
                Error::ProtocolError(error)
            })
            .unwrap_or_else(|error| {
                error!("{}", error);
                info!("Channel closed on channel {}: {:?}", self.id, method);
                Error::InvalidChannelState(ChannelState::Closing)
            });
        self.set_state(ChannelState::Closing);
        self.channel_close_ok(error)
            .try_wait()
            .transpose()
            .map(|_| ())
    }

    fn on_channel_close_ok_received(&self) -> Result<()> {
        self.set_closed(Error::InvalidChannelState(ChannelState::Closed))
    }

    fn on_queue_delete_ok_received(
        &self,
        method: protocol::queue::DeleteOk,
        pinky: Pinky<Result<LongUInt>>,
        queue: ShortString,
    ) -> Result<()> {
        self.queues.deregister(queue.as_str());
        pinky.swear(Ok(method.message_count));
        Ok(())
    }

    fn on_queue_purge_ok_received(
        &self,
        method: protocol::queue::PurgeOk,
        pinky: Pinky<Result<LongUInt>>,
    ) -> Result<()> {
        pinky.swear(Ok(method.message_count));
        Ok(())
    }

    fn on_queue_declare_ok_received(
        &self,
        method: protocol::queue::DeclareOk,
        pinky: Pinky<Result<Queue>>,
    ) -> Result<()> {
        let queue = Queue::new(method.queue, method.message_count, method.consumer_count);
        self.queues.register(queue.clone().into());
        pinky.swear(Ok(queue));
        Ok(())
    }

    fn on_basic_get_ok_received(
        &self,
        method: protocol::basic::GetOk,
        pinky: Pinky<Result<Option<BasicGetMessage>>>,
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
            pinky,
        );
        self.set_state(ChannelState::WillReceiveContent(Some(queue), None));
        Ok(())
    }

    fn on_basic_get_empty_received(&self, _: protocol::basic::GetEmpty) -> Result<()> {
        match self.connection.next_expected_reply(self.id) {
            Some(Reply::BasicGetOk(pinky, _)) => {
                pinky.swear(Ok(None));
                Ok(())
            }
            _ => {
                self.set_error(Error::UnexpectedReply)?;
                Err(Error::UnexpectedReply)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_basic_consume_ok_received(
        &self,
        method: protocol::basic::ConsumeOk,
        pinky: Pinky<Result<Consumer>>,
        queue: ShortString,
    ) -> Result<()> {
        let consumer = Consumer::new(method.consumer_tag.clone(), self.executor.clone());
        self.queues
            .register_consumer(queue.as_str(), method.consumer_tag, consumer.clone());
        pinky.swear(Ok(consumer));
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
            self.set_state(ChannelState::WillReceiveContent(
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
                    .try_wait()
                    .transpose()
                    .map(|_| ())
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
        self.set_state(ChannelState::WillReceiveContent(None, None));
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
