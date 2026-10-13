use crate::{
    BasicProperties, Result,
    acker::Acker,
    error_holder::ErrorHolder,
    internal_rpc::InternalRPCHandle,
    protocol::AMQPError,
    types::ShortString,
    types::{ChannelId, DeliveryTag, MessageCount, ReplyCode},
};
use std::ops::{Deref, DerefMut};

/// Type wrapping the output of a consumer
///
/// - Ok(Some(delivery)) carries the delivery alongside its channel
/// - Ok(None) means that the consumer got canceled
/// - Err(error) carries the error and is always followed by Ok(None)
pub type DeliveryResult = Result<Option<Delivery>>;

/// A received AMQP message.
///
/// The message has to be acknowledged after processing by calling
/// [`Acker::ack`], [`Acker::nack`] or [`Acker::reject`].
/// (Multiple acknowledgments are also possible).
#[derive(Debug, PartialEq)]
pub struct Delivery {
    /// The delivery tag of the message. Use this for
    /// acknowledging the message.
    pub delivery_tag: DeliveryTag,

    /// The exchange of the message. May be an empty string
    /// if the default exchange is used.
    pub exchange: ShortString,

    /// The routing key of the message. May be an empty string
    /// if no routing key is specified.
    pub routing_key: ShortString,

    /// Whether this message was redelivered
    pub redelivered: bool,

    /// Contains the properties and the headers of the
    /// message.
    pub properties: BasicProperties,

    /// The payload of the message in binary format.
    pub data: Vec<u8>,

    /// The acker used to ack/nack the message
    pub acker: Acker,
}

impl Delivery {
    /// Craft a new Delivery for mocking in integration testing
    #[must_use]
    pub fn mock(
        delivery_tag: DeliveryTag,
        exchange: ShortString,
        routing_key: ShortString,
        redelivered: bool,
        data: Vec<u8>,
    ) -> Self {
        let mut this = Self::new(
            0,
            delivery_tag,
            exchange,
            routing_key,
            redelivered,
            None,
            ErrorHolder::default(),
        );
        this.data = data;
        this
    }

    pub(crate) fn new(
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        exchange: ShortString,
        routing_key: ShortString,
        redelivered: bool,
        internal_rpc: Option<InternalRPCHandle>,
        poison: ErrorHolder,
    ) -> Self {
        Self {
            delivery_tag,
            exchange,
            routing_key,
            redelivered,
            properties: BasicProperties::default(),
            data: Vec::default(),
            acker: Acker::new(channel_id, delivery_tag, internal_rpc, poison),
        }
    }

    pub(crate) fn receive_content(&mut self, data: Vec<u8>) {
        self.data.extend(data);
    }
}

impl Deref for Delivery {
    type Target = Acker;

    fn deref(&self) -> &Self::Target {
        &self.acker
    }
}

/// A message retrieved via [`Channel::basic_get`].
///
/// Contains a [`Delivery`] plus the number of messages remaining in the queue.
///
/// [`Channel::basic_get`]: crate::Channel::basic_get
#[derive(Debug, PartialEq)]
pub struct BasicGetMessage {
    /// The message payload and acknowledgement handle.
    pub delivery: Delivery,
    /// Number of messages remaining in the queue after this one was retrieved.
    pub message_count: MessageCount,
}

impl BasicGetMessage {
    pub(crate) fn new(
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        exchange: ShortString,
        routing_key: ShortString,
        redelivered: bool,
        message_count: MessageCount,
        internal_rpc: InternalRPCHandle,
        poison: ErrorHolder,
    ) -> Self {
        Self {
            delivery: Delivery::new(
                channel_id,
                delivery_tag,
                exchange,
                routing_key,
                redelivered,
                Some(internal_rpc),
                poison,
            ),
            message_count,
        }
    }
}

impl Deref for BasicGetMessage {
    type Target = Delivery;

    fn deref(&self) -> &Self::Target {
        &self.delivery
    }
}

impl DerefMut for BasicGetMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.delivery
    }
}

/// A message that was published but could not be routed and was returned by the broker.
///
/// Returned messages arise from mandatory or immediate publishes where no binding
/// matched the routing key. Access them via [`Channel::wait_for_confirms`] or from
/// the [`Confirmation`] resolved by a [`PublisherConfirm`].
///
/// [`Channel::wait_for_confirms`]: crate::Channel::wait_for_confirms
/// [`Confirmation`]: crate::Confirmation
/// [`PublisherConfirm`]: crate::PublisherConfirm
#[derive(Debug, PartialEq)]
pub struct BasicReturnMessage {
    /// The returned message payload and metadata.
    pub delivery: Delivery,
    /// AMQP reply code indicating why the message was returned.
    pub reply_code: ReplyCode,
    /// Human-readable explanation for the return.
    pub reply_text: ShortString,
}

impl BasicReturnMessage {
    pub(crate) fn new(
        exchange: ShortString,
        routing_key: ShortString,
        reply_code: ReplyCode,
        reply_text: ShortString,
        poison: ErrorHolder,
    ) -> Self {
        let delivery = Delivery::new(0, 0, exchange, routing_key, false, None, poison);
        // We cannot ack a returned message
        delivery.acker.invalidate();
        Self {
            delivery,
            reply_code,
            reply_text,
        }
    }

    /// Extract the AMQP error that caused this message to be returned, if any.
    #[must_use]
    pub fn error(&self) -> Option<AMQPError> {
        AMQPError::from_id(self.reply_code, self.reply_text.clone())
    }
}

impl Deref for BasicReturnMessage {
    type Target = Delivery;

    fn deref(&self) -> &Self::Target {
        &self.delivery
    }
}

impl DerefMut for BasicReturnMessage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.delivery
    }
}
