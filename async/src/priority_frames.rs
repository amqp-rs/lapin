use amq_protocol::frame::AMQPFrame;
use parking_lot::Mutex;

use std::{
  collections::VecDeque,
  sync::Arc,
};

#[derive(Clone, Debug, Default)]
pub struct PriorityFrames {
  frames: Arc<Mutex<VecDeque<AMQPFrame>>>,
}

impl PriorityFrames {
  pub fn push_front(&self, frame: AMQPFrame) {
    self.frames.lock().push_front(frame)
  }

  pub fn push_back(&self, frame: AMQPFrame) {
    self.frames.lock().push_back(frame)
  }

  pub fn pop(&self) -> Option<AMQPFrame> {
    self.frames.lock().pop_front()
  }

  pub fn is_empty(&self) -> bool {
    self.frames.lock().is_empty()
  }
}
