use crate::{
  channel::BasicProperties,
  types::{LongLongUInt, LongUInt, ShortUInt},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
  pub delivery_tag: LongLongUInt,
  pub exchange:     String,
  pub routing_key:  String,
  pub redelivered:  bool,
  pub properties:   BasicProperties,
  pub data:         Vec<u8>,
}

impl Delivery {
  pub fn new(delivery_tag: LongLongUInt, exchange: String, routing_key: String, redelivered: bool) -> Self {
    Self {
      delivery_tag,
      exchange,
      routing_key,
      redelivered,
      properties: BasicProperties::default(),
      data:       Vec::new(),
    }
  }

  pub fn receive_content(&mut self, data: Vec<u8>) {
    self.data.extend(data);
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasicGetMessage {
  pub delivery:      Delivery,
  pub message_count: LongUInt,
}

impl BasicGetMessage {
  pub fn new(delivery_tag: LongLongUInt, exchange: String, routing_key: String, redelivered: bool, message_count: LongUInt) -> Self {
    Self {
      delivery: Delivery::new(delivery_tag, exchange, routing_key, redelivered),
      message_count,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BasicReturnMessage {
  pub delivery:   Delivery,
  pub reply_code: ShortUInt,
  pub reply_text: String,
}

impl BasicReturnMessage {
  pub fn new(exchange: String, routing_key: String, reply_code: ShortUInt, reply_text: String) -> Self {
    Self {
      delivery: Delivery::new(0, exchange, routing_key, false),
      reply_code,
      reply_text,
    }
  }
}
