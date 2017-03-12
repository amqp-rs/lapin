use format::method::Method;
use format::frame::Frame;
use std::collections::{HashMap,VecDeque};
use generated::Class;
use api::{Answer,ChannelState};
use queue::*;

#[derive(Clone,Debug,PartialEq)]
pub struct Channel<'a> {
  pub id:             u16,
  pub state:          ChannelState,
  pub frame_queue:    VecDeque<Frame>,
  pub send_flow:      bool,
  pub receive_flow:   bool,
  pub queues:         HashMap<String,Queue<'a>>,
  pub prefetch_size:  u32,
  pub prefetch_count: u16,
  pub awaiting:       VecDeque<Answer>,
}

impl<'a> Channel<'a> {
  pub fn new(channel_id: u16) -> Channel<'a> {
    Channel {
      id:             channel_id,
      state:          ChannelState::Initial,
      frame_queue:    VecDeque::new(),
      send_flow:      true,
      receive_flow:   true,
      queues:         HashMap::new(),
      prefetch_size:  0,
      prefetch_count: 0,
      awaiting:       VecDeque::new()
    }
  }

  pub fn global() -> Channel<'a> {
    Channel::new(0)
  }

  pub fn received_method(&mut self, m: Class) {
    println!("channel[{}] received {:?}", self.id, m);
    //FIXME: handle method here instead of queuing
    self.frame_queue.push_back(Frame::Method(self.id,m));
  }

  pub fn is_connected(&self) -> bool {
    self.state != ChannelState::Initial && self.state != ChannelState::Closed && self.state != ChannelState::Error
  }
}
