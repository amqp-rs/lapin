use amq_protocol::frame::AMQPFrame;
use parking_lot::Mutex;

use std::{
  collections::VecDeque,
  sync::Arc,
};

#[derive(Clone, Debug, Default)]
pub struct Frames {
  frames: Arc<Mutex<Inner>>,
}

impl Frames {
  pub fn push(&self, frame: AMQPFrame) {
    self.frames.lock().frames.push_back(frame)
  }

  pub fn push_preemptive(&self, frame: AMQPFrame) {
    self.frames.lock().priority_frames.push_front(frame)
  }

  pub fn retry(&self, frame: AMQPFrame) {
    self.frames.lock().priority_frames.push_back(frame)
  }

  pub fn pop(&self) -> Option<AMQPFrame> {
    self.frames.lock().pop()
  }

  pub fn is_empty(&self) -> bool {
    self.frames.lock().is_empty()
  }
}

#[derive(Debug, Default)]
pub struct Inner {
  priority_frames: VecDeque<AMQPFrame>,
  frames:          VecDeque<AMQPFrame>,
}

impl Inner {
  fn pop(&mut self) -> Option<AMQPFrame> {
    self.priority_frames.pop_front().or_else(|| self.frames.pop_front())
  }

  fn is_empty(&self) -> bool {
    self.priority_frames.is_empty() && self.frames.is_empty()
  }
}
