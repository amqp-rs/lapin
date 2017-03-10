use format::method::Method;
use format::frame::Frame;
use std::collections::{HashMap,VecDeque};
use std::fmt::{Debug,Formatter};
use std::fmt::Error;
use std::clone::Clone;
use generated::Class;
use api::ChannelState;
use callbacks;

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

//#[derive(Clone,Debug,PartialEq)]
pub struct Consumer<'a> {
  pub tag:         String,
  pub no_local:    bool,
  pub no_ack:      bool,
  pub exclusive:   bool,
  pub nowait:      bool,
  pub callback:    Box<callbacks::BasicConsumer + 'a>,
}

//#[derive(PartialEq)]
pub struct Queue<'a> {
  pub name:           String,
  pub passive:        bool,
  pub durable:        bool,
  pub exclusive:      bool,
  pub auto_delete:    bool,
  pub bindings:       HashMap<String, Binding>,
  pub consumers:      HashMap<String, Consumer<'a>>,
  pub message_count:  u32,
  pub consumer_count: u32,
  pub created:        bool,
  pub callback_holder: Option<Box<callbacks::BasicConsumer + 'a>>,
}

impl<'a> Queue<'a> {
  pub fn new(name: String, passive: bool, durable: bool, exclusive: bool, auto_delete: bool) -> Queue<'a> {
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
      callback_holder: None,
    }
  }
}

impl<'a> Clone for Queue<'a> {
    fn clone(&self) -> Self {
      Queue {
        name:            self.name.clone(),
        passive:         self.passive,
        durable:         self.durable,
        exclusive:       self.exclusive,
        auto_delete:     self.auto_delete,
        bindings:        self.bindings.clone(),
        consumers:       HashMap::new(), //self.consumers.clone(),
        message_count:   self.message_count,
        consumer_count:  self.consumer_count,
        created:         self.created,
        callback_holder: None,
    }
  }
}

impl<'a> Debug for Queue<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
      write!(f, "Queue {{ name: {}, passive: {}, durable: {}, exclusive: {}, auto_delete: {}, bindings: {:?}, consumers: {:?}, message_count: {}, consumer_count: {}, created: {},  }}", self.name, self.passive, self.durable, self.exclusive, self.auto_delete, self.bindings.len(),
        self.consumers.len(), self.message_count, self.consumer_count, self.created)
  }
}
impl<'a> PartialEq for Queue<'a> {
    fn eq(&self, other: &Self) -> bool {
      self.name == other.name &&
      self.passive == other.passive &&
      self.durable == other.durable &&
      self.exclusive == self.exclusive &&
      self.auto_delete == self.auto_delete &&
      //self.bindings == self.bindings &&
      //self.consumers == self.consumers &&
      self.message_count == self.message_count &&
      self.consumer_count == self.consumer_count
    }
}

