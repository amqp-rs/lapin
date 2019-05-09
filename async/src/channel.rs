pub use amq_protocol::protocol::{self, BasicProperties};

use amq_protocol::{
  protocol::{AMQPClass, AMQPError, AMQPSoftError},
  frame::{AMQPContentHeader, AMQPFrame},
};
use log::{error, info};

use crate::{
  acknowledgement::{Acknowledgements, DeliveryTag},
  channel_status::{ChannelStatus, ChannelState},
  connection::Connection,
  consumer::{Consumer, ConsumerSubscriber},
  error::{Error, ErrorKind},
  generated_names::GeneratedNames,
  id_sequence::IdSequence,
  message::{BasicGetMessage, Delivery},
  queue::Queue,
  queues::Queues,
  replies::Replies,
  requests::{Requests, RequestId},
  types::*,
};

#[derive(Clone, Debug)]
pub struct Channel {
      id:               u16,
      connection:       Connection,
  pub status:           ChannelStatus,
  pub acknowledgements: Acknowledgements,
  pub replies:          Replies,
      delivery_tag:     IdSequence<DeliveryTag>,
      request_id:       IdSequence<RequestId>,
  pub requests:         Requests,
  pub queues:           Queues,
  pub generated_names:  GeneratedNames,
}

impl Channel {
  pub fn new(channel_id: u16, connection: Connection) -> Channel {
    Channel {
      id:               channel_id,
      connection,
      status:           ChannelStatus::default(),
      acknowledgements: Acknowledgements::default(),
      replies:          Replies::default(),
      delivery_tag:     IdSequence::new(false),
      request_id:       IdSequence::new(false),
      requests:         Requests::default(),
      queues:           Queues::default(),
      generated_names:  GeneratedNames::default(),
    }
  }

  pub fn set_error(&self) -> Result<(), Error> {
    self.status.set_state(ChannelState::Error);
    self.connection.channels.remove(self.id)
  }

  pub fn set_closed(&self) -> Result<(), Error> {
    self.status.set_state(ChannelState::Closed);
    self.connection.channels.remove(self.id)
  }

  pub fn id(&self) -> u16 {
    self.id
  }

  #[doc(hidden)]
  pub fn send_method_frame(&self, method: AMQPClass) {
    self.send_frame(AMQPFrame::Method(self.id, method));
  }

  #[doc(hidden)]
  pub fn send_frame(&self, frame: AMQPFrame) {
    self.connection.send_frame(frame);
  }

  fn send_content_frames(&self, class_id: u16, slice: &[u8], properties: BasicProperties) {
    let header = AMQPContentHeader {
      class_id,
      weight:    0,
      body_size: slice.len() as u64,
      properties,
    };
    self.send_frame(AMQPFrame::Header(self.id, class_id, Box::new(header)));

    let frame_max = self.connection.configuration.frame_max();
    //a content body frame 8 bytes of overhead
    for chunk in slice.chunks(frame_max as usize - 8) {
      self.send_frame(AMQPFrame::Body(self.id, Vec::from(chunk)));
    }
  }

  #[doc(hidden)]
  pub fn handle_content_header_frame(&self, size: u64, properties: BasicProperties) -> Result<(), Error> {
    if let ChannelState::WillReceiveContent(queue_name, consumer_tag) = self.status.state() {
      if size > 0 {
        self.status.set_state(ChannelState::ReceivingContent(queue_name.clone(), consumer_tag.clone(), size as usize));
      } else {
        self.status.set_state(ChannelState::Connected);
      }
      self.queues.handle_content_header_frame(&queue_name, consumer_tag, size, properties);
      Ok(())
    } else {
      self.set_error()
    }
  }

  #[doc(hidden)]
  pub fn handle_body_frame(&self, payload: Vec<u8>) -> Result<(), Error> {
    let payload_size = payload.len();

    if let ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size) = self.status.state() {
      if remaining_size >= payload_size {
        self.queues.handle_body_frame(&queue_name, opt_consumer_tag.clone(), remaining_size, payload_size, payload);
        if remaining_size == payload_size {
          self.status.set_state(ChannelState::Connected);
        } else {
          self.status.set_state(ChannelState::ReceivingContent(queue_name, opt_consumer_tag, remaining_size - payload_size));
        }
        Ok(())
      } else {
        error!("body frame too large");
        self.set_error()
      }
    } else {
        self.set_error()
    }
  }

  fn acknowledgement_error(&self, error: Error, class_id: u16, method_id: u16) -> Result<(), Error> {
    self.channel_close(AMQPSoftError::PRECONDITIONFAILED.get_id(), "precondition failed", class_id, method_id)?;
    Err(error)
  }

  fn on_basic_publish_sent(&self, class_id: u16, payload: Vec<u8>, properties: BasicProperties) -> Option<DeliveryTag> {
    let delivery_tag = if self.status.confirm() {
      let delivery_tag = self.delivery_tag.next();
      self.acknowledgements.register_pending(delivery_tag);
      Some(delivery_tag)
    } else {
      None
    };

    self.send_content_frames(class_id, payload.as_slice(), properties);
    delivery_tag
  }

  fn on_basic_recover_async_sent(&self) {
    self.queues.drop_prefetched_messages();
  }

  fn on_basic_ack_sent(&self, multiple: bool, delivery_tag: DeliveryTag) {
    if multiple && delivery_tag == 0 {
      self.queues.drop_prefetched_messages();
    }
  }

  fn on_basic_nack_sent(&self, multiple: bool, delivery_tag: DeliveryTag) {
    if multiple && delivery_tag == 0 {
      self.queues.drop_prefetched_messages();
    }
  }

  fn on_channel_open_ok_received(&self, _method: protocol::channel::OpenOk) -> Result<(), Error> {
    self.status.set_state(ChannelState::Connected);
    Ok(())
  }

  fn on_channel_flow_received(&self, method: protocol::channel::Flow) -> Result<(), Error> {
    self.status.set_send_flow(method.active);
    self.channel_flow_ok(ChannelFlowOkOptions {active: method.active}).map(|_| ())
  }

  fn on_channel_flow_ok_received(&self, _: protocol::channel::FlowOk) -> Result<(), Error> {
    // Nothing to do here, the server just confirmed that we paused/resumed the receiving flow
    // FIXME: return method.active?
    Ok(())
  }

  fn on_channel_close_received(&self, method: protocol::channel::Close) -> Result<(), Error> {
    if let Some(error) = AMQPError::from_id(method.reply_code) {
      error!("Channel {} closed by {}:{} => {:?} => {}", self.id, method.class_id, method.method_id, error, method.reply_text);
    } else {
      info!("Channel {} closed: {:?}", self.id, method);
    }
    self.channel_close_ok()?;
    self.set_closed()
  }

  fn on_channel_close_ok_received(&self) -> Result<(), Error> {
    self.set_closed()
  }

  fn on_queue_delete_ok_received(&self, _method: protocol::queue::DeleteOk, queue: String) -> Result<(), Error> {
    //FIXME: use messages_count
    self.queues.deregister(&queue);
    Ok(())
  }

  fn on_queue_purge_ok_received(&self, _method: protocol::queue::PurgeOk) -> Result<(), Error> {
    //FIXME: use messages_count
    Ok(())
  }

  fn on_queue_declare_ok_received(&self, method: protocol::queue::DeclareOk, request_id: RequestId) -> Result<(), Error> {
    if request_id != 0 {
      self.generated_names.register(request_id, method.queue.clone());
    }
    self.queues.register(Queue::new(method.queue, method.message_count, method.consumer_count));
    Ok(())
  }

  fn on_basic_get_ok_received(&self, method: protocol::basic::GetOk, queue: String) -> Result<(), Error> {
    self.queues.start_basic_get_delivery(&queue, BasicGetMessage::new(method.delivery_tag, method.exchange, method.routing_key, method.redelivered, method.message_count));
    self.status.set_state(ChannelState::WillReceiveContent(queue, None));
    Ok(())
  }

  fn on_basic_get_empty_received(&self, _: protocol::basic::GetEmpty) -> Result<(), Error> {
    match self.replies.next() {
      Some(Reply::AwaitingBasicGetOk(request_id, _)) => {
        self.requests.finish(request_id, false);
        Ok(())
      },
      _ => {
        self.set_error()?;
        Err(ErrorKind::UnexpectedReply.into())
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  fn on_basic_consume_ok_received(&self, method: protocol::basic::ConsumeOk, request_id: RequestId, queue: String, no_local: bool, no_ack: bool, exclusive: bool, subscriber: Box<dyn ConsumerSubscriber>) -> Result<(), Error> {
    if request_id != 0 {
      self.generated_names.register(request_id, method.consumer_tag.clone());
    }
    self.queues.register_consumer(&queue, method.consumer_tag.clone(), Consumer::new(method.consumer_tag, no_local, no_ack, exclusive, subscriber));
    Ok(())
  }

  fn on_basic_deliver_received(&self, method: protocol::basic::Deliver) -> Result<(), Error> {
    if let Some(queue_name) = self.queues.start_consumer_delivery(&method.consumer_tag, Delivery::new(method.delivery_tag, method.exchange.to_string(), method.routing_key.to_string(), method.redelivered)) {
      self.status.set_state(ChannelState::WillReceiveContent(queue_name, Some(method.consumer_tag)));
    }
    Ok(())
  }

  fn on_basic_cancel_received(&self, method: protocol::basic::Cancel) -> Result<(), Error> {
    self.queues.deregister_consumer(&method.consumer_tag);
    if !method.nowait {
      self.basic_cancel_ok(&method.consumer_tag)?;
    }
    Ok(())
  }

  fn on_basic_cancel_ok_received(&self, method: protocol::basic::CancelOk) -> Result<(), Error> {
    self.queues.deregister_consumer(&method.consumer_tag);
    Ok(())
  }

  fn on_basic_ack_received(&self, method: protocol::basic::Ack) -> Result<(), Error> {
    if self.status.confirm() {
      if method.multiple {
        if method.delivery_tag > 0 {
          self.acknowledgements.ack_all_before(method.delivery_tag).or_else(|err| self.acknowledgement_error(err, method.get_amqp_class_id(), method.get_amqp_method_id()))?;
        } else {
          self.acknowledgements.ack_all_pending();
        }
      } else {
        self.acknowledgements.ack(method.delivery_tag).or_else(|err| self.acknowledgement_error(err, method.get_amqp_class_id(), method.get_amqp_method_id()))?;
      }
    }
    Ok(())
  }

  fn on_basic_nack_received(&self, method: protocol::basic::Nack) -> Result<(), Error> {
    if self.status.confirm() {
      if method.multiple {
        if method.delivery_tag > 0 {
          self.acknowledgements.nack_all_before(method.delivery_tag).or_else(|err| self.acknowledgement_error(err, method.get_amqp_class_id(), method.get_amqp_method_id()))?;
        } else {
          self.acknowledgements.nack_all_pending();
        }
      } else {
        self.acknowledgements.nack(method.delivery_tag).or_else(|err| self.acknowledgement_error(err, method.get_amqp_class_id(), method.get_amqp_method_id()))?;
      }
    }
    Ok(())
  }

  fn on_basic_return_received(&self, _method: protocol::basic::Return) -> Result<(), Error> {
    // FIXME: do something
    Ok(())
  }

  fn on_basic_recover_ok_received(&self) -> Result<(), Error> {
    self.queues.drop_prefetched_messages();
    Ok(())
  }

  fn on_confirm_select_ok_received(&self) -> Result<(), Error> {
    self.status.set_confirm();
    Ok(())
  }

  fn on_access_request_ok_received(&self, _: protocol::access::RequestOk) -> Result<(), Error> {
    Ok(())
  }
}

include!(concat!(env!("OUT_DIR"), "/channel.rs"));
