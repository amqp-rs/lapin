use crate::{
  BasicProperties,
  types::{LongLongUInt, LongUInt, ShortString, ShortUInt},
};

#[derive(Clone, Debug, PartialEq)]
#[deprecated(note = "use lapin instead")]
pub struct Delivery {
  #[deprecated(note = "use lapin instead")]
  pub delivery_tag: LongLongUInt,
  #[deprecated(note = "use lapin instead")]
  pub exchange:     ShortString,
  #[deprecated(note = "use lapin instead")]
  pub routing_key:  ShortString,
  #[deprecated(note = "use lapin instead")]
  pub redelivered:  bool,
  #[deprecated(note = "use lapin instead")]
  pub properties:   BasicProperties,
  #[deprecated(note = "use lapin instead")]
  pub data:         Vec<u8>,
}

impl Delivery {
  pub(crate) fn new(delivery_tag: LongLongUInt, exchange: ShortString, routing_key: ShortString, redelivered: bool) -> Self {
    Self {
      delivery_tag,
      exchange,
      routing_key,
      redelivered,
      properties: BasicProperties::default(),
      data:       Vec::new(),
    }
  }

  pub(crate) fn receive_content(&mut self, data: Vec<u8>) {
    self.data.extend(data);
  }
}

#[derive(Clone, Debug, PartialEq)]
#[deprecated(note = "use lapin instead")]
pub struct BasicGetMessage {
  #[deprecated(note = "use lapin instead")]
  pub delivery:      Delivery,
  #[deprecated(note = "use lapin instead")]
  pub message_count: LongUInt,
}

impl BasicGetMessage {
  pub(crate) fn new(delivery_tag: LongLongUInt, exchange: ShortString, routing_key: ShortString, redelivered: bool, message_count: LongUInt) -> Self {
    Self {
      delivery: Delivery::new(delivery_tag, exchange, routing_key, redelivered),
      message_count,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
#[deprecated(note = "use lapin instead")]
pub struct BasicReturnMessage {
  #[deprecated(note = "use lapin instead")]
  pub delivery:   Delivery,
  #[deprecated(note = "use lapin instead")]
  pub reply_code: ShortUInt,
  #[deprecated(note = "use lapin instead")]
  pub reply_text: ShortString,
}

impl BasicReturnMessage {
  pub(crate) fn new(exchange: ShortString, routing_key: ShortString, reply_code: ShortUInt, reply_text: ShortString) -> Self {
    Self {
      delivery: Delivery::new(0, exchange, routing_key, false),
      reply_code,
      reply_text,
    }
  }
}
