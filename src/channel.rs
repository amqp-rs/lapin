use format::method::Method;
use format::frame::Frame;
use std::collections::VecDeque;

#[derive(Clone,Debug,PartialEq)]
pub struct Channel {
  id:    u16,
  state: ChannelState,
  queue: VecDeque<LocalFrame>,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ChannelState {
  Waiting,
  Error,
}

#[derive(Clone,Debug,PartialEq)]
pub enum LocalFrame {
  Method(Method),
  Header,
  Heartbeat,
  Body(Vec<u8>)
}

impl Channel {
  pub fn new(channel_id: u16) -> Channel {
    Channel {
      id: channel_id,
      state: ChannelState::Waiting,
      queue: VecDeque::new(),
    }
  }

  pub fn global() -> Channel {
    Channel {
      id: 0,
      state: ChannelState::Waiting,
      queue: VecDeque::new(),
    }
  }

  pub fn received_method(&mut self, m: Method) {
    //FIXME: handle method here instead of queuing
    self.queue.push_back(LocalFrame::Method(m));
  }
}
