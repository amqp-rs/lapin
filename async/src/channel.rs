pub use amq_protocol::protocol::{self, BasicProperties};

use amq_protocol::{
  protocol::{AMQPClass, AMQPError, AMQPSoftError},
  frame::{AMQPContentHeader, AMQPFrame},
};
use either::Either;
use log::{debug, error, info, trace};

use crate::{
  acknowledgement::{Acknowledgements, DeliveryTag},
  channel_status::{ChannelStatus, ChannelState},
  connection::Connection,
  connection_status::{ConnectionState, ConnectingState},
  consumer::{Consumer, ConsumerSubscriber},
  error::{Error, ErrorKind},
  generated_names::GeneratedNames,
  id_sequence::IdSequence,
  message::{BasicGetMessage, BasicReturnMessage, Delivery},
  queue::Queue,
  queues::Queues,
  replies::Replies,
  requests::{Requests, RequestId},
  returned_messages::ReturnedMessages,
  types::*,
};

#[derive(Clone, Debug)]
pub struct Channel {
      id:                u16,
      connection:        Connection,
  pub status:            ChannelStatus,
  pub acknowledgements:  Acknowledgements,
  pub replies:           Replies,
      delivery_tag:      IdSequence<DeliveryTag>,
      request_id:        IdSequence<RequestId>,
  pub requests:          Requests,
  pub queues:            Queues,
  pub generated_names:   GeneratedNames,
      returned_messages: ReturnedMessages,
}

impl Channel {
  pub fn new(channel_id: u16, connection: Connection) -> Channel {
    Channel {
      id:                channel_id,
      connection,
      status:            ChannelStatus::default(),
      acknowledgements:  Acknowledgements::default(),
      replies:           Replies::default(),
      delivery_tag:      IdSequence::new(false),
      request_id:        IdSequence::new(false),
      requests:          Requests::default(),
      queues:            Queues::default(),
      generated_names:   GeneratedNames::default(),
      returned_messages: ReturnedMessages::default(),
    }
  }

  pub fn set_closing(&self) {
    self.status.set_state(ChannelState::Closing);
  }

  pub fn set_closed(&self) -> Result<(), Error> {
    self.status.set_state(ChannelState::Closed);
    self.connection.channels.remove(self.id)
  }

  pub fn set_error(&self) -> Result<(), Error> {
    self.status.set_state(ChannelState::Error);
    self.connection.channels.remove(self.id)
  }

  pub fn id(&self) -> u16 {
    self.id
  }

  #[doc(hidden)]
  pub fn send_method_frame(&self, method: AMQPClass) -> Result<(), Error> {
    self.send_frame(AMQPFrame::Method(self.id, method))?;
    Ok(())
  }

  #[doc(hidden)]
  pub fn send_frame(&self, frame: AMQPFrame) -> Result<(), Error> {
    self.connection.send_frame(frame)?;
    Ok(())
  }

  fn send_content_frames(&self, class_id: u16, slice: &[u8], properties: BasicProperties) -> Result<(), Error> {
    let header = AMQPContentHeader {
      class_id,
      weight:    0,
      body_size: slice.len() as u64,
      properties,
    };
    self.send_frame(AMQPFrame::Header(self.id, class_id, Box::new(header)))?;

    let frame_max = self.connection.configuration.frame_max();
    //a content body frame 8 bytes of overhead
    for chunk in slice.chunks(frame_max as usize - 8) {
      self.send_frame(AMQPFrame::Body(self.id, Vec::from(chunk)))?;
    }
    Ok(())
  }

  #[doc(hidden)]
  pub fn handle_content_header_frame(&self, size: u64, properties: BasicProperties) -> Result<(), Error> {
    if let ChannelState::WillReceiveContent(queue_name, request_id_or_consumer_tag) = self.status.state() {
      if size > 0 {
        self.status.set_state(ChannelState::ReceivingContent(queue_name.clone(), request_id_or_consumer_tag.clone(), size as usize));
      } else {
        self.status.set_state(ChannelState::Connected);
      }
      if let Some(queue_name) = queue_name {
        self.queues.handle_content_header_frame(queue_name.as_str(), request_id_or_consumer_tag, size, properties);
      } else {
        self.returned_messages.set_delivery_properties(properties);
        if size == 0 {
          self.returned_messages.new_delivery_complete();
        }
      }
      Ok(())
    } else {
      self.set_error()
    }
  }

  #[doc(hidden)]
  pub fn handle_body_frame(&self, payload: Vec<u8>) -> Result<(), Error> {
    let payload_size = payload.len();

    if let ChannelState::ReceivingContent(queue_name, request_id_or_consumer_tag, remaining_size) = self.status.state() {
      if remaining_size >= payload_size {
        if let Some(queue_name) = queue_name.as_ref() {
          self.queues.handle_body_frame(queue_name.as_str(), request_id_or_consumer_tag.clone(), remaining_size, payload_size, payload);
        } else {
          self.returned_messages.receive_delivery_content(payload);
          if remaining_size == payload_size {
            self.returned_messages.new_delivery_complete();
          }
        }
        if remaining_size == payload_size {
          self.status.set_state(ChannelState::Connected);
        } else {
          self.status.set_state(ChannelState::ReceivingContent(queue_name, request_id_or_consumer_tag, remaining_size - payload_size));
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

  fn on_connection_start_ok_sent(&self) -> Result<(), Error> {
    self.connection.status.set_connecting_state(ConnectingState::SentStartOk);
    Ok(())
  }

  fn on_connection_open_sent(&self) -> Result<(), Error> {
    self.connection.status.set_connecting_state(ConnectingState::SentOpen);
    Ok(())
  }

  fn on_connection_close_sent(&self) -> Result<(), Error> {
    self.connection.set_closing();
    Ok(())
  }

  fn on_connection_close_ok_sent(&self) -> Result<(), Error> {
    self.connection.set_closed()
  }

  fn on_channel_close_sent(&self) -> Result<(), Error> {
    self.set_closing();
    Ok(())
  }

  fn on_channel_close_ok_sent(&self) -> Result<(), Error> {
    self.set_closed()
  }

  fn on_basic_publish_sent(&self, class_id: u16, payload: Vec<u8>, properties: BasicProperties) -> Result<Option<DeliveryTag>, Error> {
    let delivery_tag = if self.status.confirm() {
      let delivery_tag = self.delivery_tag.next();
      self.acknowledgements.register_pending(delivery_tag);
      Some(delivery_tag)
    } else {
      None
    };

    self.send_content_frames(class_id, payload.as_slice(), properties)?;
    Ok(delivery_tag)
  }

  fn on_basic_recover_async_sent(&self) -> Result<(), Error> {
    self.queues.drop_prefetched_messages();
    Ok(())
  }

  fn on_basic_ack_sent(&self, multiple: bool, delivery_tag: DeliveryTag) -> Result<(), Error> {
    if multiple && delivery_tag == 0 {
      self.queues.drop_prefetched_messages();
    }
    Ok(())
  }

  fn on_basic_nack_sent(&self, multiple: bool, delivery_tag: DeliveryTag) -> Result<(), Error> {
    if multiple && delivery_tag == 0 {
      self.queues.drop_prefetched_messages();
    }
    Ok(())
  }

  fn tune_connection_configuration(&self, channel_max: u16, frame_max: u32, heartbeat: u16) {
    // If we disable the heartbeat (0) but the server don't, follow him and enable it too
    // If both us and the server want heartbeat enabled, pick the lowest value.
    if self.connection.configuration.heartbeat() == 0 || heartbeat != 0 && heartbeat < self.connection.configuration.heartbeat() {
      self.connection.configuration.set_heartbeat(heartbeat);
    }

    if channel_max != 0 {
      // 0 means we want to take the server's value
      // If both us and the server specified a channel_max, pick the lowest value.
      if self.connection.configuration.channel_max() == 0 || channel_max < self.connection.configuration.channel_max() {
        self.connection.configuration.set_channel_max(channel_max);
      }
    }
    if self.connection.configuration.channel_max() == 0 {
      self.connection.configuration.set_channel_max(u16::max_value());
    }

    if frame_max != 0 {
      // 0 means we want to take the server's value
      // If both us and the server specified a frame_max, pick the lowest value.
      if self.connection.configuration.frame_max() == 0 || frame_max < self.connection.configuration.frame_max() {
        self.connection.configuration.set_frame_max(frame_max);
      }
    }
    if self.connection.configuration.frame_max() == 0 {
      self.connection.configuration.set_frame_max(u32::max_value());
    }
  }

  fn on_connection_start_received(&self, method: protocol::connection::Start) -> Result<(), Error> {
    trace!("Server sent connection::Start: {:?}", method);
    let state = self.connection.status.state();
    if let ConnectionState::Connecting(ConnectingState::SentProtocolHeader(credentials, mut options)) = state {
      let mechanism = options.mechanism.to_string();
      let locale    = options.locale.clone();

      if !method.mechanisms.split_whitespace().any(|m| m == mechanism) {
        error!("unsupported mechanism: {}", mechanism);
      }
      if !method.locales.split_whitespace().any(|l| l == locale) {
        error!("unsupported locale: {}", mechanism);
      }

      if !options.client_properties.contains_key("product") || !options.client_properties.contains_key("version") {
        options.client_properties.insert("product".into(), AMQPValue::LongString(env!("CARGO_PKG_NAME").into()));
        options.client_properties.insert("version".into(), AMQPValue::LongString(env!("CARGO_PKG_VERSION").into()));
      }

      options.client_properties.insert("platform".into(), AMQPValue::LongString("rust".into()));

      let mut capabilities = FieldTable::default();
      capabilities.insert("publisher_confirms".into(), AMQPValue::Boolean(true));
      capabilities.insert("exchange_exchange_bindings".into(), AMQPValue::Boolean(true));
      capabilities.insert("basic.nack".into(), AMQPValue::Boolean(true));
      capabilities.insert("consumer_cancel_notify".into(), AMQPValue::Boolean(true));
      capabilities.insert("connection.blocked".into(), AMQPValue::Boolean(true));
      capabilities.insert("authentication_failure_close".into(), AMQPValue::Boolean(true));

      options.client_properties.insert("capabilities".into(), AMQPValue::FieldTable(capabilities));

      self.connection_start_ok(options.client_properties, &mechanism, &credentials.sasl_plain_auth_string(), &locale)?;
      Ok(())
    } else {
      error!("Invalid state: {:?}", state);
      self.connection.set_error()?;
      Err(ErrorKind::InvalidConnectionState(state).into())
    }
  }

  fn on_connection_tune_received(&self, method: protocol::connection::Tune) -> Result<(), Error> {
    debug!("Server sent Connection::Tune: {:?}", method);

    self.tune_connection_configuration(method.channel_max, method.frame_max, method.heartbeat);

    self.connection_tune_ok(self.connection.configuration.channel_max(), self.connection.configuration.frame_max(), self.connection.configuration.heartbeat())?;
    self.connection_open(&self.connection.status.vhost())?;
    Ok(())
  }

  fn on_connection_open_ok_received(&self, _: protocol::connection::OpenOk) -> Result<(), Error> {
    self.connection.status.set_state(ConnectionState::Connected);
    Ok(())
  }

  fn on_connection_close_received(&self, method: protocol::connection::Close) -> Result<(), Error> {
    if let Some(error) = AMQPError::from_id(method.reply_code) {
      error!("Connection closed on channel {} by {}:{} => {:?} => {}", self.id, method.class_id, method.method_id, error, method.reply_text);
    } else {
      info!("Connection closed on channel {}: {:?}", self.id, method);
    }
    self.connection_close_ok()?;
    Ok(())
  }

  fn on_connection_blocked_received(&self, _method: protocol::connection::Blocked) -> Result<(), Error> {
    self.connection.status.block();
    Ok(())
  }

  fn on_connection_unblocked_received(&self, _method: protocol::connection::Unblocked) -> Result<(), Error> {
    self.connection.status.unblock();
    Ok(())
  }

  fn on_connection_close_ok_received(&self) -> Result<(), Error> {
    self.connection.set_closed()
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
    Ok(())
  }

  fn on_channel_close_ok_received(&self) -> Result<(), Error> {
    self.set_closed()
  }

  fn on_queue_delete_ok_received(&self, _method: protocol::queue::DeleteOk, queue: ShortString) -> Result<(), Error> {
    //FIXME: use messages_count
    self.queues.deregister(queue.as_str());
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

  fn on_basic_get_ok_received(&self, method: protocol::basic::GetOk, request_id: RequestId, queue: ShortString) -> Result<(), Error> {
    self.queues.start_basic_get_delivery(queue.as_str(), BasicGetMessage::new(method.delivery_tag, method.exchange, method.routing_key, method.redelivered, method.message_count));
    self.status.set_state(ChannelState::WillReceiveContent(Some(queue), Either::Left(request_id)));
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
  fn on_basic_consume_ok_received(&self, method: protocol::basic::ConsumeOk, request_id: RequestId, queue: ShortString, no_local: bool, no_ack: bool, exclusive: bool, subscriber: Box<dyn ConsumerSubscriber>) -> Result<(), Error> {
    if request_id != 0 {
      self.generated_names.register(request_id, method.consumer_tag.clone());
    }
    self.queues.register_consumer(queue.as_str(), method.consumer_tag.clone(), Consumer::new(method.consumer_tag, no_local, no_ack, exclusive, subscriber));
    Ok(())
  }

  fn on_basic_deliver_received(&self, method: protocol::basic::Deliver) -> Result<(), Error> {
    if let Some(queue_name) = self.queues.start_consumer_delivery(method.consumer_tag.as_str(), Delivery::new(method.delivery_tag, method.exchange.into(), method.routing_key.into(), method.redelivered)) {
      self.status.set_state(ChannelState::WillReceiveContent(Some(queue_name), Either::Right(method.consumer_tag)));
    }
    Ok(())
  }

  fn on_basic_cancel_received(&self, method: protocol::basic::Cancel) -> Result<(), Error> {
    self.queues.deregister_consumer(method.consumer_tag.as_str());
    if !method.nowait {
      self.basic_cancel_ok(method.consumer_tag.as_str())?;
    }
    Ok(())
  }

  fn on_basic_cancel_ok_received(&self, method: protocol::basic::CancelOk) -> Result<(), Error> {
    self.queues.deregister_consumer(method.consumer_tag.as_str());
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

  fn on_basic_return_received(&self, method: protocol::basic::Return) -> Result<(), Error> {
    self.returned_messages.start_new_delivery(BasicReturnMessage::new(method.exchange, method.routing_key, method.reply_code, method.reply_text));
    self.status.set_state(ChannelState::WillReceiveContent(None, Either::Left(0)));
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
