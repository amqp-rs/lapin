use format::method::Method;
use format::frame::Frame;
use std::collections::{HashMap,VecDeque};
use generated::Class;
use api::ChannelState;


#[derive(Clone,Debug,PartialEq)]
pub struct Channel {
  pub id:           u16,
  pub state:        ChannelState,
  pub frame_queue:  VecDeque<LocalFrame>,
  pub send_flow:    bool,
  pub receive_flow: bool,
  pub queues:       HashMap<String,Queue>,
}

/*
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ChannelState {
  Waiting,
  Error,
}
*/

#[derive(Clone,Debug,PartialEq)]
pub enum LocalFrame {
  Method(Class),
  Header,
  Heartbeat,
  Body(Vec<u8>)
}

impl Channel {
  pub fn new(channel_id: u16) -> Channel {
    Channel {
      id:           channel_id,
      state:        ChannelState::Initial,
      frame_queue:  VecDeque::new(),
      send_flow:    true,
      receive_flow: true,
      queues:       HashMap::new(),
    }
  }

  pub fn global() -> Channel {
    Channel::new(0)
  }

  pub fn received_method(&mut self, m: Class) {
    println!("channel[{}] received {:?}", self.id, m);
    //FIXME: handle method here instead of queuing
    self.frame_queue.push_back(LocalFrame::Method(m));
  }
}

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
pub struct Queue {
  pub name:           String,
  pub passive:        bool,
  pub durable:        bool,
  pub exclusive:      bool,
  pub auto_delete:    bool,
  pub bindings:       HashMap<String, Binding>,
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
      message_count:  0,
      consumer_count: 0,
      created:        false,
    }
  }
}
