use log::trace;
use parking_lot::Mutex;

use std::{
  collections::VecDeque,
  sync::Arc,
};

use crate::channel::Reply;

#[derive(Clone, Debug, Default)]
pub struct Replies {
  replies: Arc<Mutex<VecDeque<Reply>>>,
}

impl Replies {
  pub fn register_pending(&self, channel_id: u16, reply: Reply) {
    trace!("channel {} state is now waiting for {:?}", channel_id, reply);
    self.replies.lock().push_back(reply);
  }

  pub fn next(&self) -> Option<Reply> {
    self.replies.lock().pop_front()
  }
}
