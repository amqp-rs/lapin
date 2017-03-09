use format::method::Method;
use format::frame::Frame;
use std::collections::VecDeque;
use generated::Class;
use api::ChannelState;

#[derive(Clone,Debug,PartialEq)]
pub struct Channel {
  pub id:           u16,
  pub state:        ChannelState,
  pub queue:        VecDeque<LocalFrame>,
  pub send_flow:    bool,
  pub receive_flow: bool,
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
      id: channel_id,
      state: ChannelState::Initial,
      queue: VecDeque::new(),
      send_flow: true,
      receive_flow: true,
    }
  }

  pub fn global() -> Channel {
    Channel::new(0)
  }

  pub fn received_method(&mut self, m: Class) {
    println!("channel[{}] received {:?}", self.id, m);
    //FIXME: handle method here instead of queuing
    self.queue.push_back(LocalFrame::Method(m));
  }
}
