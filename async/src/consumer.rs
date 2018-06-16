use std::collections::VecDeque;
use message::Delivery;

#[derive(Clone,Debug,PartialEq)]
pub struct Consumer {
      tag:             String,
      no_local:        bool,
      no_ack:          bool,
      exclusive:       bool,
      nowait:          bool,
      messages:        VecDeque<Delivery>,
  pub current_message: Option<Delivery>,
}

impl Consumer {
  pub fn new(tag: String, no_local: bool, no_ack: bool, exclusive: bool, nowait: bool) -> Consumer {
    Consumer {
      tag,
      no_local,
      no_ack,
      exclusive,
      nowait,
      messages: VecDeque::new(),
      current_message: None,
    }
  }

  pub fn new_delivery_complete(&mut self) {
    if let Some(delivery) = self.current_message.take() {
      self.messages.push_back(delivery);
    }
  }

  pub fn next_delivery(&mut self) -> Option<Delivery> {
    self.messages.pop_front()
  }

  pub fn has_deliveries(&self) -> bool {
    !self.messages.is_empty()
  }
}
