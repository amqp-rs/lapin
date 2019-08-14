use log::trace;
use amq_protocol::frame::AMQPFrame;
use parking_lot::Mutex;

use std::{
  collections::{VecDeque, HashMap},
  sync::Arc,
};

use crate::{
  channel::Reply,
  channel_status::ChannelState,
  id_sequence::IdSequence,
  wait::{Cancellable, Wait, WaitHandle},
  error::ErrorKind,
};

pub(crate) type SendId = u64;

#[derive(Clone, Debug)]
pub(crate) enum Priority {
  NORMAL,
  CRITICAL,
}

impl Default for Priority {
  fn default() -> Self {
    Priority::NORMAL
  }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Frames {
  inner: Arc<Mutex<Inner>>,
}

impl Frames {
  pub(crate) fn push(&self, channel_id: u16, priority: Priority, frame: AMQPFrame, expected_reply: Option<(Reply, Box<dyn Cancellable + Send>)>) -> Wait<()> {
    self.inner.lock().push(channel_id, priority, frame, expected_reply)
  }

  pub(crate) fn push_frames(&self, channel_id: u16, frames: Vec<(AMQPFrame, Option<AMQPFrame>)>) -> Wait<()> {
    self.inner.lock().push_frames(channel_id, frames)
  }

  pub(crate) fn retry(&self, send_id: SendId, frame: AMQPFrame) {
    self.inner.lock().retry(send_id, frame);
  }

  pub(crate) fn pop(&self, flow: bool) -> Option<(SendId, AMQPFrame)> {
    self.inner.lock().pop(flow)
  }

  pub(crate) fn next_expected_reply(&self, channel_id: u16) -> Option<Reply> {
    self.inner.lock().expected_replies.get_mut(&channel_id).and_then(|replies| replies.pop_front()).map(|t| t.0)
  }

  pub(crate) fn mark_sent(&self, send_id: SendId) {
    if let Some((_, send)) = self.inner.lock().outbox.remove(&send_id) {
      send.finish(());
    }
  }

  pub(crate) fn drop_pending(&self) {
    self.inner.lock().drop_pending();
  }

  pub(crate) fn clear_expected_replies(&self, channel_id: u16, channel_state: ChannelState) {
    self.inner.lock().clear_expected_replies(channel_id, channel_state);
  }
}

#[derive(Debug)]
struct Inner {
  next_frame:       Option<(SendId, AMQPFrame)>,
  priority_frames:  VecDeque<(SendId, AMQPFrame)>,
  frames:           VecDeque<(SendId, AMQPFrame)>,
  low_prio_frames:  VecDeque<(SendId, AMQPFrame, Option<AMQPFrame>)>,
  expected_replies: HashMap<u16, VecDeque<(Reply, Box<dyn Cancellable + Send>)>>,
  outbox:           HashMap<SendId, (u16, WaitHandle<()>)>,
  send_id:          IdSequence<SendId>,
}

impl Default for Inner {
  fn default() -> Self {
    Self {
      next_frame:       None,
      priority_frames:  VecDeque::default(),
      frames:           VecDeque::default(),
      low_prio_frames:  VecDeque::default(),
      expected_replies: HashMap::default(),
      outbox:           HashMap::default(),
      send_id:          IdSequence::new(false),
    }
  }
}

impl Inner {
  fn push(&mut self, channel_id: u16, priority: Priority, frame: AMQPFrame, expected_reply: Option<(Reply, Box<dyn Cancellable + Send>)>) -> Wait<()> {
    let send_id = if let Priority::CRITICAL = priority { 0 } else { self.send_id.next() };
    match priority {
      Priority::NORMAL   => self.frames.push_back((send_id, frame)),
      Priority::CRITICAL => self.priority_frames.push_front((send_id, frame)),
    }
    let (wait, wait_handle) = Wait::new();
    self.outbox.insert(send_id, (channel_id, wait_handle));
    if let Some(reply) = expected_reply {
      trace!("channel {} state is now waiting for {:?}", channel_id, reply);
      self.expected_replies.entry(channel_id).or_default().push_back(reply);
    }
    wait
  }

  fn push_frames(&mut self, channel_id: u16, mut frames: Vec<(AMQPFrame, Option<AMQPFrame>)>) -> Wait<()> {
    let send_id = self.send_id.next();
    let (wait, wait_handle) = Wait::new();
    let last_frame = frames.pop();

    for frame in frames {
      self.low_prio_frames.push_back((0, frame.0, frame.1));
    }
    if let Some(last_frame) = last_frame {
      self.low_prio_frames.push_back((send_id, last_frame.0, last_frame.1));
    } else {
      wait_handle.finish(());
    }

    self.outbox.insert(send_id, (channel_id, wait_handle));

    wait
  }

  fn pop(&mut self, flow: bool) -> Option<(SendId, AMQPFrame)> {
    if let Some(frame) = self.next_frame.take().or_else(|| self.priority_frames.pop_front()).or_else(|| self.frames.pop_front()) {
      return Some(frame);
    }
    if flow {
      if let Some(mut frame) = self.low_prio_frames.pop_front() {
        if let Some(next_frame) = frame.2 {
          self.next_frame = Some((frame.0, next_frame));
          frame.0 = 0;
        }
        return Some((frame.0, frame.1));
      }
    }
    None
  }

  fn retry(&mut self, send_id: SendId, frame: AMQPFrame) {
    if let AMQPFrame::Header(..) = &frame {
      self.next_frame = Some((send_id, frame));
    } else {
      self.priority_frames.push_back((send_id, frame));
    }
  }

  fn drop_pending(&mut self) {
    self.priority_frames.clear();
    self.frames.clear();
    self.low_prio_frames.clear();
    for (_, replies) in self.expected_replies.drain() {
      Self::cancel_expected_replies(replies, ChannelState::Closed);
    }
    for (_, (_, wait_handle)) in self.outbox.drain() {
      wait_handle.finish(());
    }
  }

  fn clear_expected_replies(&mut self, channel_id: u16, channel_state: ChannelState) {
    let mut outbox = HashMap::default();

    for (send_id, (chan_id, wait_handle)) in self.outbox.drain() {
      if chan_id == channel_id {
        wait_handle.error(ErrorKind::InvalidChannelState(channel_state.clone()).into())
      } else {
        outbox.insert(send_id, (chan_id, wait_handle));
      }
    }

    self.outbox = outbox;

    if let Some(replies) = self.expected_replies.remove(&channel_id) {
      Self::cancel_expected_replies(replies, channel_state);
    }
  }

  fn cancel_expected_replies(replies: VecDeque<(Reply, Box<dyn Cancellable + Send>)>, channel_state: ChannelState) {
    for (_, cancel) in replies {
      cancel.cancel(ErrorKind::InvalidChannelState(channel_state.clone()).into());
    }
  }
}
