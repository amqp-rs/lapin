use crate::channel::BasicProperties;
use crate::types::{LongLongUInt, LongUInt};

#[derive(Clone,Debug,PartialEq)]
pub struct Delivery {
  pub delivery_tag: LongLongUInt,
  pub exchange:     String,
  pub routing_key:  String,
  pub redelivered:  bool,
  pub properties:   BasicProperties,
  pub data:         Vec<u8>,
}

impl Delivery {
  pub fn new(delivery_tag: LongLongUInt, exchange: String, routing_key: String, redelivered: bool) -> Delivery {
    Delivery {
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

#[derive(Clone,Debug,PartialEq)]
pub struct BasicGetMessage {
  pub delivery:      Delivery,
  pub message_count: LongUInt,
}

impl BasicGetMessage {
  pub fn new(delivery_tag: LongLongUInt, exchange: String, routing_key: String, redelivered: bool, message_count: LongUInt) -> BasicGetMessage {
    BasicGetMessage {
      delivery: Delivery::new(delivery_tag, exchange, routing_key, redelivered),
      message_count,
    }
  }
}
