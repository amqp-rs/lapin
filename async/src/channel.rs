pub use amq_protocol::protocol::BasicProperties;

use amq_protocol::frame::AMQPFrame;
use crossbeam_channel::Sender;
use log::error;

use std::result;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::api::{Answer, ChannelState};
use crate::error::{Error, ErrorKind};
use crate::message::BasicGetMessage;
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
  pub confirm:        bool,
  pub acked:          HashSet<u64>,
  pub nacked:         HashSet<u64>,
  pub unacked:        HashSet<u64>,
  pub awaiting:       VecDeque<Answer>,
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
      confirm:        false,
      acked:          HashSet::new(),
      nacked:         HashSet::new(),
      unacked:        HashSet::new(),
      awaiting:       VecDeque::new(),
      delivery_tag:   1,
      frame_sender,
    }
  }

  /// verifies if the channel's state is the one passed as argument
  pub fn check_state(&self, state: ChannelState) -> result::Result<(), Error> {
    if self.state == state {
      Ok(())
    } else {
      Err(ErrorKind::InvalidState {
        expected: state,
        actual:   self.state.clone(),
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
    if let ChannelState::WillReceiveContent(queue_name, consumer_tag) = self.state.clone() {
      if size > 0 {
        self.state = ChannelState::ReceivingContent(queue_name.clone(), consumer_tag.clone(), size as usize);
      } else {
        self.state = ChannelState::Connected;
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
      self.state = ChannelState::Error;
    }
  }

  #[doc(hidden)]
  pub fn handle_body_frame(&mut self, payload: Vec<u8>) {
    let payload_size = payload.len();

    if let ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size) = self.state.clone() {
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
          self.state = ChannelState::Connected;
        } else {
          self.state = ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size - payload_size);
        }
      } else {
        error!("body frame too large");
        self.state = ChannelState::Error;
      }
    } else {
      self.state = ChannelState::Error;
    }
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
