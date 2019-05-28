use log::trace;
use amq_protocol::frame::AMQPFrame;
use parking_lot::Mutex;

use std::{
  collections::{VecDeque, HashMap},
  sync::Arc,
};

use crate::channel::Reply;

#[derive(Clone, Debug, Default)]
pub struct Frames {
  inner: Arc<Mutex<Inner>>,
}

impl Frames {
  pub fn push(&self, channel_id: u16, frame: AMQPFrame, expected_reply: Option<Reply>) {
    self.inner.lock().push(channel_id, frame, expected_reply)
  }

  pub fn push_preemptive(&self, frame: AMQPFrame) {
    self.inner.lock().priority_frames.push_front(frame)
  }

  pub fn retry(&self, frame: AMQPFrame) {
    self.inner.lock().priority_frames.push_back(frame)
  }

  pub fn pop(&self) -> Option<AMQPFrame> {
    self.inner.lock().pop()
  }

  pub fn is_empty(&self) -> bool {
    self.inner.lock().is_empty()
  }

  pub fn next_expected_reply(&self, channel_id: u16) -> Option<Reply> {
    // FIXME: pop all replies on channel close
    self.inner.lock().expected_replies.get_mut(&channel_id).and_then(|replies| replies.pop_front())
  }
}

#[derive(Debug, Default)]
pub struct Inner {
  priority_frames:  VecDeque<AMQPFrame>,
  frames:           VecDeque<AMQPFrame>,
  expected_replies: HashMap<u16, VecDeque<Reply>>,
}

impl Inner {
  fn push(&mut self, channel_id: u16, frame: AMQPFrame, expected_reply: Option<Reply>) {
    self.frames.push_back(frame);
    if let Some(reply) = expected_reply {
      trace!("channel {} state is now waiting for {:?}", channel_id, reply);
      self.expected_replies.entry(channel_id).or_default().push_back(reply);
    }
  }

  fn pop(&mut self) -> Option<AMQPFrame> {
    self.priority_frames.pop_front().or_else(|| self.frames.pop_front())
  }

  fn is_empty(&self) -> bool {
    self.priority_frames.is_empty() && self.frames.is_empty()
  }
}
