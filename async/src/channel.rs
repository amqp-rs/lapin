pub use amq_protocol::protocol::{self, BasicProperties};

use amq_protocol::protocol::AMQPClass;
use amq_protocol::frame::{AMQPContentHeader, AMQPFrame};
use crossbeam_channel::{self, Receiver, Sender};
use log::{error, trace};
use parking_lot::{Mutex, RwLock};

use std::collections::{HashMap, HashSet};
use std::result;
use std::sync::Arc;

use crate::api::{Answer, ChannelState};
use crate::connection::Configuration;
use crate::consumer::{Consumer, ConsumerSubscriber};
use crate::error::{Error, ErrorKind};
use crate::message::{BasicGetMessage, Delivery};
use crate::queue::Queue;
use crate::types::*;

pub type DeliveryTag = u64;
pub type RequestId = u64;

#[derive(Clone, Debug)]
pub struct ChannelHandle {
  pub id:              u16,
      configuration:   Arc<RwLock<Configuration>>,
      confirm:         Arc<RwLock<bool>>,
      state:           Arc<RwLock<ChannelState>>,
      request_index:   Arc<Mutex<RequestId>>,
      delivery_tag:    Arc<Mutex<DeliveryTag>>,
      unacked:         Arc<Mutex<HashSet<DeliveryTag>>>,
      queues:          Arc<Mutex<HashMap<String, Queue>>>,
      frame_sender:    Sender<AMQPFrame>,
      awaiting_sender: Sender<Answer>,
}

impl ChannelHandle {
  pub fn await_answer(&mut self, answer: Answer) {
    trace!("channel {} state is now waiting for {:?}", self.id, answer);
    // We always hold a reference to the receiver so it's safe to unwrap
    self.awaiting_sender.send(answer).unwrap()
  }

  pub fn await_ack(&mut self, delivery_tag: DeliveryTag) {
    let mut unacked = self.unacked.lock();
    unacked.insert(delivery_tag);
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

  pub fn is_initializing(&self) -> bool {
    let current_state = self.state.read();
    *current_state == ChannelState::Initial
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
    if *request_index == 0 {
      *request_index += 1;
    }
    id
  }

  #[doc(hidden)]
  pub fn next_delivery_tag(&mut self) -> u64 {
    let mut delivery_tag = self.delivery_tag.lock();
    let tag = *delivery_tag;;
    *delivery_tag += 1;
    if *delivery_tag == 0 {
      *delivery_tag += 1;
    }
    tag
  }

  pub fn confirm(&self) -> bool {
    let confirm = self.confirm.read();
    *confirm
  }

  fn on_basic_publish_sent(&mut self, class_id: u16, payload: Vec<u8>, properties: BasicProperties) {
    if self.confirm() {
      self.await_answer(Answer::AwaitingPublishConfirm(0));
      let delivery_tag = self.next_delivery_tag();
      self.await_ack(delivery_tag);
    }
    self.send_content_frames(class_id, payload.as_slice(), properties)
  }

  fn on_basic_recover_async_sent(&mut self) {
    self.drop_prefetched_messages();
  }

  fn on_basic_ack_sent(&mut self, multiple: bool, delivery_tag: DeliveryTag) {
    if multiple && delivery_tag == 0 {
      self.drop_prefetched_messages();
    }
  }

  fn on_basic_nack_sent(&mut self, multiple: bool, delivery_tag: DeliveryTag) {
    if multiple && delivery_tag == 0 {
      self.drop_prefetched_messages();
    }
  }

  pub fn drop_prefetched_messages(&mut self) {
    let mut queues = self.queues.lock();
    for queue in queues.values_mut() {
      queue.drop_prefetched_messages();
    }
  }

  pub fn get_queue_stats(&self, queue: &str) -> (u32, u32) {
    let queues = self.queues.lock();
    if let Some(queue) = queues.get(queue) {
      (queue.consumer_count, queue.message_count)
    } else {
      (0, 0)
    }
  }

  fn send_content_frames(&mut self, class_id: u16, slice: &[u8], properties: BasicProperties) {
    let header = AMQPContentHeader {
      class_id,
      weight:    0,
      body_size: slice.len() as u64,
      properties,
    };
    self.send_frame(AMQPFrame::Header(self.id, class_id, Box::new(header)));

    let frame_max = {
      let configuration = self.configuration.read();
      configuration.frame_max
    };

    //a content body frame 8 bytes of overhead
    for chunk in slice.chunks(frame_max as usize - 8) {
      self.send_frame(AMQPFrame::Body(self.id, Vec::from(chunk)));
    }
  }
}

#[derive(Debug)]
pub struct Channel {
  pub id:              u16,
  pub send_flow:       bool,
  pub receive_flow:    bool,
  pub prefetch_count:  u16,
  pub acked:           HashSet<DeliveryTag>,
  pub nacked:          HashSet<DeliveryTag>,
  pub awaiting:        Receiver<Answer>,
      configuration:   Arc<RwLock<Configuration>>,
      confirm:         Arc<RwLock<bool>>,
      state:           Arc<RwLock<ChannelState>>,
      request_index:   Arc<Mutex<RequestId>>,
      delivery_tag:    Arc<Mutex<DeliveryTag>>,
      unacked:         Arc<Mutex<HashSet<DeliveryTag>>>,
      queues:          Arc<Mutex<HashMap<String, Queue>>>,
      frame_sender:    Sender<AMQPFrame>,
      awaiting_sender: Sender<Answer>,
}

impl Channel {
  pub fn new(channel_id: u16, configuration: Arc<RwLock<Configuration>>, frame_sender: Sender<AMQPFrame>, request_index: Arc<Mutex<RequestId>>) -> Channel {
    let (awaiting_sender, awaiting_receiver) = crossbeam_channel::unbounded();

    Channel {
      id:             channel_id,
      configuration,
      send_flow:      true,
      receive_flow:   true,
      prefetch_count: 0,
      acked:          HashSet::new(),
      nacked:         HashSet::new(),
      awaiting:       awaiting_receiver,
      confirm:        Arc::new(RwLock::new(false)),
      state:          Arc::new(RwLock::new(ChannelState::Initial)),
      request_index,
      delivery_tag:   Arc::new(Mutex::new(1)),
      queues:         Arc::new(Mutex::new(HashMap::new())),
      frame_sender,
      unacked:        Arc::new(Mutex::new(HashSet::new())),
      awaiting_sender,
    }
  }

  pub fn handle(&self) -> ChannelHandle {
    ChannelHandle {
      id:              self.id,
      configuration:   self.configuration.clone(),
      confirm:         self.confirm.clone(),
      state:           self.state.clone(),
      request_index:   self.request_index.clone(),
      delivery_tag:    self.delivery_tag.clone(),
      unacked:         self.unacked.clone(),
      queues:          self.queues.clone(),
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

  pub fn drain_all_unacked(&mut self) -> HashSet<DeliveryTag> {
    let mut unacked = self.unacked.lock();
    unacked.drain().collect()
  }

  pub fn all_unacked_before(&mut self, delivery_tag: DeliveryTag) -> HashSet<DeliveryTag> {
    let unacked = self.unacked.lock();
    unacked.iter().filter(|tag| *tag <= &delivery_tag).cloned().collect()
  }

  pub fn confirm(&self) -> bool {
    let confirm = self.confirm.read();
    *confirm
  }

  pub fn set_confirm(&mut self) {
    let mut confirm = self.confirm.write();
    *confirm = true;
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
    let mut queues = self.queues.lock();
    queues.get_mut(queue_name).and_then(|queue| queue.next_basic_get_message())
  }

  #[doc(hidden)]
  pub fn handle_content_header_frame(&mut self, size: u64, properties: BasicProperties) {
    if let ChannelState::WillReceiveContent(queue_name, consumer_tag) = self.state() {
      if size > 0 {
        self.set_state(ChannelState::ReceivingContent(queue_name.clone(), consumer_tag.clone(), size as usize));
      } else {
        self.set_state(ChannelState::Connected);
      }
      let mut queues = self.queues.lock();
      if let Some(ref mut q) = queues.get_mut(&queue_name) {
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
        {
          let mut queues = self.queues.lock();
          if let Some(ref mut q) = queues.get_mut(&queue_name) {
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
    let mut delivery_tag = self.delivery_tag.lock();
    let tag = *delivery_tag;;
    *delivery_tag += 1;
    if *delivery_tag == 0 {
      *delivery_tag += 1;
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
    let mut unacked = self.unacked.lock();
    if !unacked.remove(&delivery_tag) {
      return Err(ErrorKind::PreconditionFailed.into());
    }
    Ok(())
  }

  pub fn register_queue(&mut self, queue: Queue) {
    let mut queues = self.queues.lock();
    queues.insert(queue.name.clone(), queue);
  }

  pub fn deregister_queue(&mut self, queue: &str) {
    let mut queues = self.queues.lock();
    queues.remove(queue);
  }

  pub fn register_consumer(&mut self, queue: &str, consumer_tag: &str, consumer: Consumer) {
    let mut queues = self.queues.lock();
    queues.get_mut(queue).map(|q| {
      q.consumers.insert(consumer_tag.to_string(), consumer)
    });
  }

  pub fn deregister_consumer(&mut self, consumer_tag: &str) {
    let mut queues = self.queues.lock();
    for queue in queues.values_mut() {
      queue.consumers.remove(consumer_tag).map(|mut consumer| consumer.cancel());
    }
  }

  pub fn start_consumer_delivery(&mut self, consumer_tag: &str, message: Delivery) {
    let mut queue_name = None;
    {
      let mut queues = self.queues.lock();
      for queue in queues.values_mut() {
        if let Some(consumer) = queue.consumers.get_mut(consumer_tag) {
          consumer.start_new_delivery(message);
          queue_name = Some(queue.name.clone());
          break;
        }
      }
    }
    if let Some(queue_name) = queue_name.take() {
      self.set_state(ChannelState::WillReceiveContent(queue_name, Some(consumer_tag.to_string())));
    }
  }

  pub fn start_basic_get_delivery(&mut self, queue: &str, message: BasicGetMessage) {
    let mut queues = self.queues.lock();
    if let Some(q) = queues.get_mut(queue) {
      q.start_new_delivery(message);
    }
  }
}

include!(concat!(env!("OUT_DIR"), "/channel.rs"));
