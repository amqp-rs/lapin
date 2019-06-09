pub use lapin_async::channel::BasicProperties;
pub use lapin_async::channel::options::*;

use futures::{Future, future};
use lapin_async;
use lapin_async::channel::Channel as InnerChannel;
use lapin_async::connection::Connection;
use lapin_async::queue::Queue;
use log::trace;

use crate::confirmation::ConfirmationFuture;
use crate::consumer::Consumer;
use crate::error::Error;
use crate::message::{BasicGetMessage, BasicReturnMessage};
use crate::types::*;

/// `Channel` provides methods to act on a channel, such as managing queues
#[derive(Clone)]
pub struct Channel {
  inner: InnerChannel,
}

impl Channel {
  /// create a channel
  pub fn create(conn: &Connection) -> impl Future<Item = Self, Error = Error> {
    let confirmation: ConfirmationFuture<InnerChannel> = conn.create_channel().into();
    confirmation.map(|inner| Channel { inner })
  }

  pub fn id(&self) -> u16 {
    self.inner.id()
  }

  /// request access
  ///
  /// returns a future that resolves once the access is granted
  pub fn access_request(&self, realm: &str, options: AccessRequestOptions) -> ConfirmationFuture<()> {
    self.inner.access_request(realm, options).into()
  }

  /// declares an exchange
  ///
  /// returns a future that resolves once the exchange is available
  pub fn exchange_declare(&self, name: &str, exchange_type: &str, options: ExchangeDeclareOptions, arguments: FieldTable) -> ConfirmationFuture<()> {
    self.inner.exchange_declare(name, exchange_type, options, arguments).into()
  }

  /// deletes an exchange
  ///
  /// returns a future that resolves once the exchange is deleted
  pub fn exchange_delete(&self, name: &str, options: ExchangeDeleteOptions) -> ConfirmationFuture<()> {
    self.inner.exchange_delete(name, options).into()
  }

  /// binds an exchange to another exchange
  ///
  /// returns a future that resolves once the exchanges are bound
  pub fn exchange_bind(&self, destination: &str, source: &str, routing_key: &str, options: ExchangeBindOptions, arguments: FieldTable) -> ConfirmationFuture<()> {
    self.inner.exchange_bind(destination, source, routing_key, options, arguments).into()
  }

  /// unbinds an exchange from another one
  ///
  /// returns a future that resolves once the exchanges are unbound
  pub fn exchange_unbind(&self, destination: &str, source: &str, routing_key: &str, options: ExchangeUnbindOptions, arguments: FieldTable) -> ConfirmationFuture<()> {
    self.inner.exchange_unbind(destination, source, routing_key, options, arguments).into()
  }

  /// declares a queue
  ///
  /// returns a future that resolves once the queue is available
  ///
  /// the `mandatory` and `ìmmediate` options can be set to true,
  /// but the return message will not be handled
  pub fn queue_declare(&self, name: &str, options: QueueDeclareOptions, arguments: FieldTable) -> ConfirmationFuture<Queue> {
    self.inner.queue_declare(name, options, arguments).into()
  }

  /// binds a queue to an exchange
  ///
  /// returns a future that resolves once the queue is bound to the exchange
  pub fn queue_bind(&self, name: &str, exchange: &str, routing_key: &str, options: QueueBindOptions, arguments: FieldTable) -> ConfirmationFuture<()> {
    self.inner.queue_bind(name, exchange, routing_key, options, arguments).into()
  }

  /// unbinds a queue from the exchange
  ///
  /// returns a future that resolves once the queue is unbound from the exchange
  pub fn queue_unbind(&self, name: &str, exchange: &str, routing_key: &str, arguments: FieldTable) -> ConfirmationFuture<()> {
    self.inner.queue_unbind(name, exchange, routing_key, arguments).into()
  }

  /// sets up confirm extension for this channel
  pub fn confirm_select(&self, options: ConfirmSelectOptions) -> ConfirmationFuture<()> {
    self.inner.confirm_select(options).into()
  }

  /// specifies quality of service for a channel
  pub fn basic_qos(&self, prefetch_count: ShortUInt, options: BasicQosOptions) -> ConfirmationFuture<()> {
    self.inner.basic_qos(prefetch_count, options).into()
  }

  /// publishes a message on a queue
  ///
  /// the future's result is:
  /// - `Some(true)` if we're on a confirm channel and the message was ack'd
  /// - `Some(false)` if we're on a confirm channel and the message was nack'd
  /// - `None` if we're not on a confirm channel
  pub fn basic_publish(&self, exchange: &str, routing_key: &str, payload: Vec<u8>, options: BasicPublishOptions, properties: BasicProperties) -> ConfirmationFuture<()> {
    self.inner.basic_publish(exchange, routing_key, options, payload, properties).into()
  }

  /// creates a consumer stream
  ///
  /// returns a future of a `Consumer` that resolves once the method succeeds
  ///
  /// `Consumer` implements `futures::Stream`, so it can be used with any of
  /// the usual combinators
  pub fn basic_consume(&self, queue: &Queue, consumer_tag: &str, options: BasicConsumeOptions, arguments: FieldTable) -> impl Future<Item = Consumer, Error = Error> {
    let mut consumer = Consumer::new(self.id(), queue.name.clone(), consumer_tag.into());
    let subscriber = consumer.subscriber();
    let confirmation: ConfirmationFuture<ShortString> = self.inner.basic_consume(queue.name.as_str(), consumer_tag, options, arguments, Box::new(subscriber)).into();
    confirmation.map(|consumer_tag| {
      trace!("basic_consume received response, returning consumer");
      consumer.update_consumer_tag(consumer_tag);
      consumer
    })
  }

  pub fn basic_cancel(&self, consumer_tag: &str, options: BasicCancelOptions) -> ConfirmationFuture<()> {
    self.inner.basic_cancel(consumer_tag, options).into()
  }

  pub fn basic_recover(&self, options: BasicRecoverOptions) -> ConfirmationFuture<()> {
    self.inner.basic_recover(options).into()
  }

  pub fn basic_recover_async(&self, options: BasicRecoverAsyncOptions) -> ConfirmationFuture<()> {
    self.inner.basic_recover_async(options).into()
  }

  /// acks a message
  pub fn basic_ack(&self, delivery_tag: u64, multiple: bool) -> ConfirmationFuture<()> {
    self.inner.basic_ack(delivery_tag, BasicAckOptions { multiple }).into()
  }

  /// nacks a message
  pub fn basic_nack(&self, delivery_tag: u64, multiple: bool, requeue: bool) -> ConfirmationFuture<()> {
    self.inner.basic_nack(delivery_tag, BasicNackOptions { multiple, requeue }).into()
  }

  /// rejects a message
  pub fn basic_reject(&self, delivery_tag: u64, options: BasicRejectOptions) -> ConfirmationFuture<()> {
    self.inner.basic_reject(delivery_tag, options).into()
  }

  /// gets a message
  pub fn basic_get(&self, queue: &str, options: BasicGetOptions) -> ConfirmationFuture<Option<BasicGetMessage>> {
    self.inner.basic_get(queue, options).into()
  }

  /// Purge a queue.
  ///
  /// This method removes all messages from a queue which are not awaiting acknowledgment.
  pub fn queue_purge(&self, queue_name: &str, options: QueuePurgeOptions) -> ConfirmationFuture<LongUInt> {
    self.inner.queue_purge(queue_name, options).into()
  }

  /// Delete a queue.
  ///
  /// This method deletes a queue. When a queue is deleted any pending messages are sent to a dead-letter queue
  /// if this is defined in the server configuration, and all consumers on the queue are cancelled.
  ///
  /// If `if_unused` is set, the server will only delete the queue if it has no consumers.
  /// If the queue has consumers the server does not delete it but raises a channel exception instead.
  ///
  /// If `if_empty` is set, the server will only delete the queue if it has no messages.
  pub fn queue_delete(&self, queue_name: &str, options: QueueDeleteOptions) -> ConfirmationFuture<LongUInt> {
    self.inner.queue_delete(queue_name, options).into()
  }

  /// closes the channel
  pub fn close(&self, code: u16, message: &str) -> ConfirmationFuture<()> {
    self.inner.channel_close(code, message, 0, 0).into()
  }

  /// ack a channel close
  pub fn close_ok(&self) -> ConfirmationFuture<()> {
    self.inner.channel_close_ok().into()
  }

  /// update a channel flow
  pub fn channel_flow(&self, options: ChannelFlowOptions) -> ConfirmationFuture<Boolean> {
    self.inner.channel_flow(options).into()
  }

  /// ack an update to a channel flow
  pub fn channel_flow_ok(&self, options: ChannelFlowOkOptions) -> ConfirmationFuture<()> {
    self.inner.channel_flow_ok(options).into()
  }

  pub fn tx_select(&self) -> ConfirmationFuture<()> {
    self.inner.tx_select().into()
  }

  pub fn tx_commit(&self) -> ConfirmationFuture<()> {
    self.inner.tx_commit().into()
  }

  pub fn tx_rollback(&self) -> ConfirmationFuture<()> {
    self.inner.tx_rollback().into()
  }

  pub fn wait_for_confirms(&self) -> impl Future<Item = Vec<BasicReturnMessage>, Error = Error> {
    // TODO: make async
    future::ok(self.inner.wait_for_confirms())
  }
}
