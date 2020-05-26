use crate::{
    types::{LongLongUInt, LongUInt, ShortString, ShortUInt},
    BasicProperties, Channel, Result,
};

/// Type wrapping the output of a consumer
///
/// - Ok(Some((channel, delivery))) carries the delivery alongside its channel
/// - Ok(None) means that the consumer got canceled
/// - Err(error) carries the error and is always followed by Ok(None)
pub type DeliveryResult = Result<Option<(Channel, Delivery)>>;


/// A received AMQP message.
///
/// The message has to be acknowledged after processing by calling
/// [`Channel::basic_ack`], [`Channel::basic_reject`] or [`Channel::basic_nack`] with the delivery tag.
/// (Multiple acknowledgments are also possible).
///
/// It is important to acknowledge on the same channel where the message was received.
///
/// [`Channel::basic_ack`]: ../struct.Channel.html#method.basic_ack
/// [`Channel::basic_reject`]: ../struct.Channel.html#method.basic_reject
/// [`Channel::basic_nack`]: ../struct.Channel.html#method.basic_nack
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    /// The delivery tag of the message. Use this for
    /// acknowledging the message.
    pub delivery_tag: LongLongUInt,

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
}

impl Delivery {
    pub(crate) fn new(
        delivery_tag: LongLongUInt,
        exchange: ShortString,
        routing_key: ShortString,
        redelivered: bool,
    ) -> Self {
        Self {
            delivery_tag,
            exchange,
            routing_key,
            redelivered,
            properties: BasicProperties::default(),
            data: Vec::default(),
        }
    }

    pub(crate) fn receive_content(&mut self, data: Vec<u8>) {
        self.data.extend(data);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasicGetMessage {
    pub delivery: Delivery,
    pub message_count: LongUInt,
}

impl BasicGetMessage {
    pub(crate) fn new(
        delivery_tag: LongLongUInt,
        exchange: ShortString,
        routing_key: ShortString,
        redelivered: bool,
        message_count: LongUInt,
    ) -> Self {
        Self {
            delivery: Delivery::new(delivery_tag, exchange, routing_key, redelivered),
            message_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasicReturnMessage {
    pub delivery: Delivery,
    pub reply_code: ShortUInt,
    pub reply_text: ShortString,
}

impl BasicReturnMessage {
    pub(crate) fn new(
        exchange: ShortString,
        routing_key: ShortString,
        reply_code: ShortUInt,
        reply_text: ShortString,
    ) -> Self {
        Self {
            delivery: Delivery::new(0, exchange, routing_key, false),
            reply_code,
            reply_text,
        }
    }
}
