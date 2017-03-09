use format::method::Method;
use format::frame::Frame;
use std::collections::{HashMap,VecDeque};
use generated::Class;
use api::ChannelState;

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

#[derive(Clone,Debug,PartialEq)]
pub struct Consumer {
  pub tag:         String,
  pub no_local:    bool,
  pub no_ack:      bool,
  pub exclusive:   bool,
  pub nowait:      bool,
}

#[derive(Clone,Debug,PartialEq)]
pub struct Queue {
  pub name:           String,
  pub passive:        bool,
  pub durable:        bool,
  pub exclusive:      bool,
  pub auto_delete:    bool,
  pub bindings:       HashMap<String, Binding>,
  pub consumers:      HashMap<String, Consumer>,
  pub message_count:  u32,
  pub consumer_count: u32,
  pub created:        bool,
}

impl Queue {
  pub fn new(name: String, passive: bool, durable: bool, exclusive: bool, auto_delete: bool) -> Queue {
    Queue {
      name:           name,
      passive:        passive,
      durable:        durable,
      exclusive:      exclusive,
      auto_delete:    auto_delete,
      bindings:       HashMap::new(),
      consumers:      HashMap::new(),
      message_count:  0,
      consumer_count: 0,
      created:        false,
    }
  }
}
