use log::trace;
use amq_protocol::frame::AMQPFrame;
use parking_lot::Mutex;

use std::{
  collections::{VecDeque, HashMap, HashSet},
  sync::Arc,
};

use crate::{
  channel::Reply,
  id_sequence::IdSequence,
};

pub type SendId = u64;

#[derive(Clone, Debug, Default)]
pub struct Frames {
  inner: Arc<Mutex<Inner>>,
}

impl Frames {
  pub fn push(&self, channel_id: u16, frame: AMQPFrame, expected_reply: Option<Reply>) -> SendId {
    self.inner.lock().push(channel_id, frame, expected_reply)
  }

  pub fn push_preemptive(&self, frame: AMQPFrame) {
    self.inner.lock().priority_frames.push_front((9, frame))
  }

  pub fn retry(&self, send_id: SendId, frame: AMQPFrame) {
    self.inner.lock().priority_frames.push_back((send_id, frame))
  }

  pub fn pop(&self) -> Option<(SendId, AMQPFrame)> {
    self.inner.lock().pop()
  }

  pub fn is_empty(&self) -> bool {
    self.inner.lock().is_empty()
  }

  pub fn next_expected_reply(&self, channel_id: u16) -> Option<Reply> {
    // FIXME: pop all replies on channel close
    self.inner.lock().expected_replies.get_mut(&channel_id).and_then(|replies| replies.pop_front())
  }

  pub fn is_pending(&self, send_id: SendId) -> bool {
    self.inner.lock().outbox.contains(&send_id)
  }

  pub fn mark_sent(&self, send_id: SendId) {
    self.inner.lock().outbox.remove(&send_id);
  }
}

#[derive(Debug)]
pub struct Inner {
  priority_frames:  VecDeque<(SendId, AMQPFrame)>,
  frames:           VecDeque<(SendId, AMQPFrame)>,
  expected_replies: HashMap<u16, VecDeque<Reply>>,
  outbox:           HashSet<SendId>,
  send_id:          IdSequence<SendId>,
}

impl Default for Inner {
  fn default() -> Self {
    Self {
      priority_frames:  VecDeque::default(),
      frames:           VecDeque::default(),
      expected_replies: HashMap::default(),
      outbox:           HashSet::default(),
      send_id:          IdSequence::new(false),
    }
  }
}

impl Inner {
  fn push(&mut self, channel_id: u16, frame: AMQPFrame, expected_reply: Option<Reply>) -> SendId {
    let send_id = self.send_id.next();
    self.frames.push_back((send_id, frame));
    self.outbox.insert(send_id);
    if let Some(reply) = expected_reply {
      trace!("channel {} state is now waiting for {:?}", channel_id, reply);
      self.expected_replies.entry(channel_id).or_default().push_back(reply);
    }
    send_id
  }

  fn pop(&mut self) -> Option<(SendId, AMQPFrame)> {
    self.priority_frames.pop_front().or_else(|| self.frames.pop_front())
  }

  fn is_empty(&self) -> bool {
    self.priority_frames.is_empty() && self.frames.is_empty()
  }
}
