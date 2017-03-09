use format::method::Method;
use format::frame::Frame;
use std::collections::{HashMap,VecDeque};
use generated::Class;
use api::ChannelState;
use queue::*;

#[derive(Clone,Debug,PartialEq)]
pub struct Channel {
  pub id:             u16,
  pub state:          ChannelState,
  pub frame_queue:    VecDeque<LocalFrame>,
  pub send_flow:      bool,
  pub receive_flow:   bool,
  pub queues:         HashMap<String,Queue>,
  pub prefetch_size:  u32,
  pub prefetch_count: u16,
}

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
      id:             channel_id,
      state:          ChannelState::Initial,
      frame_queue:    VecDeque::new(),
      send_flow:      true,
      receive_flow:   true,
      queues:         HashMap::new(),
      prefetch_size:  0,
      prefetch_count: 0,
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
