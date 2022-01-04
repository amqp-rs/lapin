use crate::{
    acker::Acker,
    error_holder::ErrorHolder,
    internal_rpc::InternalRPCHandle,
    protocol::AMQPError,
    types::ShortString,
    types::{ChannelId, DeliveryTag, MessageCount, ReplyCode},
    BasicProperties, Result,
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
///
/// [`Acker::ack`]: ../struct.Acker.html#method.ack
/// [`Acker::nack`]: ../struct.Acker.html#method.nack
/// [`Acker::reject`]: ../struct.Acker.html#method.reject
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
    pub(crate) fn new(
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        exchange: ShortString,
        routing_key: ShortString,
        redelivered: bool,
        internal_rpc: Option<InternalRPCHandle>,
        error: Option<ErrorHolder>,
    ) -> Self {
        Self {
            delivery_tag,
            exchange,
            routing_key,
            redelivered,
            properties: BasicProperties::default(),
            data: Vec::default(),
            acker: Acker::new(channel_id, delivery_tag, internal_rpc, error),
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

#[derive(Debug, PartialEq)]
pub struct BasicGetMessage {
    pub delivery: Delivery,
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
    ) -> Self {
        Self {
            delivery: Delivery::new(
                channel_id,
                delivery_tag,
                exchange,
                routing_key,
                redelivered,
                Some(internal_rpc),
                None,
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

#[derive(Debug, PartialEq)]
pub struct BasicReturnMessage {
    pub delivery: Delivery,
    pub reply_code: ReplyCode,
    pub reply_text: ShortString,
}

impl BasicReturnMessage {
    pub(crate) fn new(
        exchange: ShortString,
        routing_key: ShortString,
        reply_code: ReplyCode,
        reply_text: ShortString,
    ) -> Self {
        Self {
            delivery: Delivery::new(0, 0, exchange, routing_key, false, None, None),
            reply_code,
            reply_text,
        }
    }

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
