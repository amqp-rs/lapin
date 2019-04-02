pub use amq_protocol::protocol::BasicProperties;

use amq_protocol::protocol::AMQPClass;
use amq_protocol::frame::AMQPFrame;
use crossbeam_channel::{self, Receiver, Sender};
use log::error;
use parking_lot::{Mutex, RwLock};

use std::collections::{HashMap, HashSet};
use std::result;
use std::sync::Arc;

use crate::api::{Answer, ChannelState};
use crate::consumer::ConsumerSubscriber;
use crate::error::{Error, ErrorKind};
use crate::message::BasicGetMessage;
use crate::queue::Queue;

pub type RequestId = u64;

#[derive(Clone, Debug)]
pub struct ChannelHandle {
  pub id:              u16,
      state:           Arc<RwLock<ChannelState>>,
      request_index:   Arc<Mutex<RequestId>>,
      frame_sender:    Sender<AMQPFrame>,
      awaiting_sender: Sender<Answer>,
}

impl ChannelHandle {
  pub fn await_answer(&mut self, answer: Answer) {
    // We always hold a reference to the receiver so it's safe to unwrap
    self.awaiting_sender.send(answer).unwrap()
  }

  #[doc(hidden)]
  pub fn send_method_frame(&mut self, method: AMQPClass) {
    self.send_frame(AMQPFrame::Method(self.id, method));
  }

  #[doc(hidden)]
  pub fn send_frame(&mut self, frame: AMQPFrame) {
    // We always hold a reference to the receiver so it's safe to unwrap
    self.frame_sender.send(frame).unwrap();
  }

  pub fn is_connected(&self) -> bool {
    let current_state = self.state.read();
    *current_state != ChannelState::Initial && *current_state != ChannelState::Closed && *current_state != ChannelState::Error
  }

  #[doc(hidden)]
  pub fn next_request_id(&mut self) -> RequestId {
    let mut request_index = self.request_index.lock();
    let id = *request_index;
    *request_index += 1;
    id
  }
}

#[derive(Debug)]
pub struct Channel {
  pub id:              u16,
  pub send_flow:       bool,
  pub receive_flow:    bool,
  pub queues:          HashMap<String, Queue>,
  pub prefetch_size:   u32,
  pub prefetch_count:  u16,
  pub confirm:         bool,
  pub acked:           HashSet<u64>,
  pub nacked:          HashSet<u64>,
  pub unacked:         HashSet<u64>,
  pub awaiting:        Receiver<Answer>,
      state:           Arc<RwLock<ChannelState>>,
      request_index:   Arc<Mutex<RequestId>>,
      delivery_tag:    u64,
      frame_sender:    Sender<AMQPFrame>,
      awaiting_sender: Sender<Answer>,
}

impl Channel {
  pub fn new(channel_id: u16, frame_sender: Sender<AMQPFrame>, request_index: Arc<Mutex<RequestId>>) -> Channel {
    let (awaiting_sender, awaiting_receiver) = crossbeam_channel::unbounded();

    Channel {
      id:             channel_id,
      send_flow:      true,
      receive_flow:   true,
      queues:         HashMap::new(),
      prefetch_size:  0,
      prefetch_count: 0,
      confirm:        false,
      acked:          HashSet::new(),
      nacked:         HashSet::new(),
      unacked:        HashSet::new(),
      awaiting:       awaiting_receiver,
      state:          Arc::new(RwLock::new(ChannelState::Initial)),
      request_index,
      delivery_tag:   1,
      frame_sender,
      awaiting_sender,
    }
  }

  pub fn handle(&self) -> ChannelHandle {
    ChannelHandle {
      id:              self.id,
      state:           self.state.clone(),
      request_index:   self.request_index.clone(),
      frame_sender:    self.frame_sender.clone(),
      awaiting_sender: self.awaiting_sender.clone(),
    }
  }

  pub fn await_answer(&mut self, answer: Answer) {
    // We always hold a reference to the receiver so it's safe to unwrap
    self.awaiting_sender.send(answer).unwrap()
  }

  pub fn next_answer(&mut self) -> Option<Answer> {
      // Error means no message
    self.awaiting.try_recv().ok()
  }

  pub fn state(&self) -> ChannelState {
    self.state.read().clone()
  }

  pub fn set_state(&mut self, state: ChannelState) {
    let mut current_state = self.state.write();
    *current_state = state;
  }

  /// verifies if the channel's state is the one passed as argument
  pub fn check_state(&self, state: ChannelState) -> result::Result<(), Error> {
    let current_state = self.state();
    if current_state == state {
      Ok(())
    } else {
      Err(ErrorKind::InvalidState {
        expected: state,
        actual:   current_state,
      }.into())
    }
  }

  /// gets the next message corresponding to a queue, in response to a basic.get
  ///
  /// If there is no message, the method will return None
  pub fn next_basic_get_message(&mut self, queue_name: &str) -> Option<BasicGetMessage> {
      self.queues.get_mut(queue_name).and_then(|queue| queue.next_basic_get_message())
  }

  #[doc(hidden)]
  pub fn handle_content_header_frame(&mut self, size: u64, properties: BasicProperties) {
    if let ChannelState::WillReceiveContent(queue_name, consumer_tag) = self.state() {
      if size > 0 {
        self.set_state(ChannelState::ReceivingContent(queue_name.clone(), consumer_tag.clone(), size as usize));
      } else {
        self.set_state(ChannelState::Connected);
      }
      if let Some(ref mut q) = self.queues.get_mut(&queue_name) {
        if let Some(ref consumer_tag) = consumer_tag {
          if let Some(ref mut cs) = q.consumers.get_mut(consumer_tag) {
            cs.set_delivery_properties(properties);
            if size == 0 {
              cs.new_delivery_complete();
            }
          }
        } else {
          q.set_delivery_properties(properties);
          if size == 0 {
            q.new_delivery_complete();
          }
        }
      }
    } else {
      self.set_state(ChannelState::Error);
    }
  }

  #[doc(hidden)]
  pub fn handle_body_frame(&mut self, payload: Vec<u8>) {
    let payload_size = payload.len();

    if let ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size) = self.state() {
      if remaining_size >= payload_size {
        if let Some(ref mut q) = self.queues.get_mut(&queue_name) {
          if let Some(ref consumer_tag) = opt_consumer_tag {
            if let Some(ref mut cs) = q.consumers.get_mut(consumer_tag) {
              cs.receive_delivery_content(payload);
              if remaining_size == payload_size {
                cs.new_delivery_complete();
              }
            }
          } else {
            q.receive_delivery_content(payload);
            if remaining_size == payload_size {
              q.new_delivery_complete();
            }
          }
        }

        if remaining_size == payload_size {
          self.set_state(ChannelState::Connected);
        } else {
          self.set_state(ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size - payload_size));
        }
      } else {
        error!("body frame too large");
        self.set_state(ChannelState::Error);
      }
    } else {
      self.set_state(ChannelState::Error);
    }
  }

  pub fn is_connected(&self) -> bool {
    let current_state = self.state.read();
    *current_state != ChannelState::Initial && *current_state != ChannelState::Closed && *current_state != ChannelState::Error
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
