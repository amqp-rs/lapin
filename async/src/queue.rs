use std::collections::{HashMap,VecDeque};
use consumer::Consumer;
use message::*;

#[derive(Clone,Debug,PartialEq)]
pub struct Binding {
  pub exchange:    String,
  pub routing_key: String,
  pub no_wait:     bool,
  pub active:      bool,
}

impl Binding {
  pub fn new(exchange: String, routing_key: String, no_wait: bool) -> Binding {
    Binding {
      exchange:    exchange,
      routing_key: routing_key,
      no_wait:     no_wait,
      active:      false,
    }
  }
}

#[derive(Debug)]
pub struct Queue {
  pub name:                String,
  pub bindings:            HashMap<(String, String), Binding>,
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
      bindings:            HashMap::new(),
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

