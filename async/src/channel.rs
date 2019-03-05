pub use amq_protocol::protocol::BasicProperties;

use amq_protocol::frame::AMQPFrame;
use crossbeam_channel::Sender;

use std::collections::{HashMap, HashSet, VecDeque};

use crate::api::{Answer, ChannelState};
use crate::error::{Error, ErrorKind};
use crate::queue::Queue;

#[derive(Debug)]
pub struct Channel {
  pub id:             u16,
  pub state:          ChannelState,
  pub send_flow:      bool,
  pub receive_flow:   bool,
  pub queues:         HashMap<String, Queue>,
  pub prefetch_size:  u32,
  pub prefetch_count: u16,
  pub awaiting:       VecDeque<Answer>,
  pub confirm:        bool,
  pub acked:          HashSet<u64>,
  pub nacked:         HashSet<u64>,
  pub unacked:        HashSet<u64>,
      delivery_tag:   u64,
      frame_sender:   Sender<AMQPFrame>,
}

impl Channel {
  pub fn new(channel_id: u16, frame_sender: Sender<AMQPFrame>) -> Channel {
    Channel {
      id:             channel_id,
      state:          ChannelState::Initial,
      send_flow:      true,
      receive_flow:   true,
      queues:         HashMap::new(),
      prefetch_size:  0,
      prefetch_count: 0,
      awaiting:       VecDeque::new(),
      confirm:        false,
      acked:          HashSet::new(),
      nacked:         HashSet::new(),
      unacked:        HashSet::new(),
      delivery_tag:   1,
      frame_sender,
    }
  }

  pub fn global(frame_sender: Sender<AMQPFrame>) -> Channel {
    Channel::new(0, frame_sender)
  }

  pub fn is_connected(&self) -> bool {
    self.state != ChannelState::Initial && self.state != ChannelState::Closed && self.state != ChannelState::Error
  }

  pub fn next_delivery_tag(&mut self) -> u64 {
    let tag = self.delivery_tag;
    self.delivery_tag += 1;
    if self.delivery_tag == 0 {
      self.delivery_tag = 1;
    }
    tag
  }

  pub fn ack(&mut self, delivery_tag: u64) -> Result<(), Error> {
    self.drop_unacked(delivery_tag)?;
    self.acked.insert(delivery_tag);
    Ok(())
  }

  pub fn nack(&mut self, delivery_tag: u64) -> Result<(), Error> {
    self.drop_unacked(delivery_tag)?;
    self.nacked.insert(delivery_tag);
    Ok(())
  }

  fn drop_unacked(&mut self, delivery_tag: u64) -> Result<(), Error> {
    if !self.unacked.remove(&delivery_tag) {
      return Err(ErrorKind::PreconditionFailed.into());
    }
    Ok(())
  }
}
