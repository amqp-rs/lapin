use std::collections::{HashMap, VecDeque};

use crate::consumer::Consumer;
use crate::message::BasicGetMessage;

#[derive(Debug)]
pub struct Queue {
  pub name:                String,
  pub consumers:           HashMap<String, Consumer>,
  pub message_count:       u32,
  pub consumer_count:      u32,
  pub get_messages:        VecDeque<BasicGetMessage>,
  pub current_get_message: Option<BasicGetMessage>,
}

impl Queue {
  pub fn new(name: String, message_count: u32, consumer_count: u32) -> Queue {
    Queue {
      name,
      consumers:           HashMap::new(),
      message_count,
      consumer_count,
      get_messages:        VecDeque::new(),
      current_get_message: None,
    }
  }

  pub fn next_basic_get_message(&mut self) -> Option<BasicGetMessage> {
    self.get_messages.pop_front()
  }
}
