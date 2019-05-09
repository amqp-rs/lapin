pub use lapin_async::channel::BasicProperties;
pub use lapin_async::channel::options::*;

use futures::{Async, Future, future, Poll, Stream, task};
use lapin_async;
use lapin_async::channel::Channel as InnerChannel;
use lapin_async::channel_status::ChannelState;
use lapin_async::connection::Connection;
use lapin_async::acknowledgement::DeliveryTag;
use lapin_async::queue::QueueStats;
use lapin_async::requests::RequestId;
use log::{debug, trace};
use parking_lot::Mutex;
use tokio_io::{AsyncRead, AsyncWrite};

use std::sync::Arc;

use crate::consumer::Consumer;
use crate::error::{Error, ErrorKind};
use crate::message::BasicGetMessage;
use crate::queue::Queue;
use crate::transport::AMQPTransport;
use crate::types::*;

pub type RequestResult = Result<Option<RequestId>, lapin_async::error::Error>;

/// `Channel` provides methods to act on a channel, such as managing queues
//#[derive(Clone)]
pub struct Channel<T> {
  pub transport: Arc<Mutex<AMQPTransport<T>>>,
      conn:      Connection,
      inner:     InnerChannel,
}

impl<T> Clone for Channel<T>
where T: Send {
  fn clone(&self) -> Channel<T> {
    Channel {
      transport: self.transport.clone(),
      conn:      self.conn.clone(),
      inner:     self.inner.clone(),
    }
  }
}

impl<T: AsyncRead+AsyncWrite+Send+Sync+'static> Channel<T> {
    /// create a channel
    pub fn create(transport: Arc<Mutex<AMQPTransport<T>>>, conn: Connection) -> impl Future<Item = Self, Error = Error> + Send + 'static {
        future::result(conn.create_channel().map(|inner| Channel { transport, inner, conn }).map_err(|err| ErrorKind::ProtocolError("Failed to create channel".to_string(), err).into())).and_then(|channel| {
            let request_id = channel.inner.channel_open();
            let inner = channel.inner.clone();
            channel.run_on_locked_transport("create", "Could not create channel", request_id).and_then(move |_| {
                future::poll_fn(move || {
                    match inner.status.state() {
                        ChannelState::Connected => Ok(Async::Ready(())),
                        ChannelState::Error | ChannelState::Closed => {
                            Err(ErrorKind::ChannelOpenFailed.into())
                        },
                        _ => {
                            task::current().notify();
                            Ok(Async::NotReady)
                        }
                    }
                })
            }).map(move |_| {
                channel
            })
        })
    }

    pub fn id(&self) -> u16 {
      self.inner.id()
    }

    /// request access
    ///
    /// returns a future that resolves once the access is granted
    pub fn access_request(&self, realm: &str, options: AccessRequestOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.access_request(realm, options);

        self.run_on_locked_transport("access_request", "Could not request access", request_id).map(|_| ())
    }

    /// declares an exchange
    ///
    /// returns a future that resolves once the exchange is available
    pub fn exchange_declare(&self, name: &str, exchange_type: &str, options: ExchangeDeclareOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.exchange_declare(name, exchange_type, options, arguments);

        self.run_on_locked_transport("exchange_declare", "Could not declare exchange", request_id).map(|_| ())
    }

    /// deletes an exchange
    ///
    /// returns a future that resolves once the exchange is deleted
    pub fn exchange_delete(&self, name: &str, options: ExchangeDeleteOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.exchange_delete(name, options);

        self.run_on_locked_transport("exchange_delete", "Could not delete exchange", request_id).map(|_| ())
    }

    /// binds an exchange to another exchange
    ///
    /// returns a future that resolves once the exchanges are bound
    pub fn exchange_bind(&self, destination: &str, source: &str, routing_key: &str, options: ExchangeBindOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.exchange_bind(destination, source, routing_key, options, arguments);

        self.run_on_locked_transport("exchange_bind", "Could not bind exchange", request_id).map(|_| ())
    }

    /// unbinds an exchange from another one
    ///
    /// returns a future that resolves once the exchanges are unbound
    pub fn exchange_unbind(&self, destination: &str, source: &str, routing_key: &str, options: ExchangeUnbindOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.exchange_unbind(destination, source, routing_key, options, arguments);

        self.run_on_locked_transport("exchange_unbind", "Could not unbind exchange", request_id).map(|_| ())
    }

    /// declares a queue
    ///
    /// returns a future that resolves once the queue is available
    ///
    /// the `mandatory` and `ìmmediate` options can be set to true,
    /// but the return message will not be handled
    pub fn queue_declare(&self, name: &str, options: QueueDeclareOptions, arguments: FieldTable) -> impl Future<Item = Queue, Error = Error> + Send + 'static {
        let request_id = self.inner.queue_declare(name, options, arguments);
        let inner = self.inner.clone();

        self.run_on_locked_transport("queue_declare", "Could not declare queue", request_id).and_then(move |request_id| {
            future::poll_fn(move || {
              if let Some(queue) = inner.generated_names.get(request_id.expect("expected request_id")) {
                let QueueStats {consumer_count, message_count} = inner.queues.get_stats(&queue);
                return Ok(Async::Ready(Queue::new(queue, consumer_count, message_count)))
              } else {
                task::current().notify();
                return Ok(Async::NotReady)
              }
            })
        })
    }

    /// binds a queue to an exchange
    ///
    /// returns a future that resolves once the queue is bound to the exchange
    pub fn queue_bind(&self, name: &str, exchange: &str, routing_key: &str, options: QueueBindOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.queue_bind(name, exchange, routing_key, options, arguments);

        self.run_on_locked_transport("queue_bind", "Could not bind queue", request_id).map(|_| ())
    }

    /// unbinds a queue from the exchange
    ///
    /// returns a future that resolves once the queue is unbound from the exchange
    pub fn queue_unbind(&self, name: &str, exchange: &str, routing_key: &str, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.queue_unbind(name, exchange, routing_key, arguments);

        self.run_on_locked_transport("queue_unbind", "Could not unbind queue from the exchange", request_id).map(|_| ())
    }

    /// sets up confirm extension for this channel
    pub fn confirm_select(&self, options: ConfirmSelectOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.confirm_select(options);

        self.run_on_locked_transport("confirm_select", "Could not activate confirm extension", request_id).map(|_| ())
    }

    /// specifies quality of service for a channel
    pub fn basic_qos(&self, prefetch_count: ShortUInt, options: BasicQosOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.basic_qos(prefetch_count, options);

        self.run_on_locked_transport("basic_qos", "Could not setup qos", request_id).map(|_| ())
    }

    /// publishes a message on a queue
    ///
    /// the future's result is:
    /// - `Some(true)` if we're on a confirm channel and the message was ack'd
    /// - `Some(false)` if we're on a confirm channel and the message was nack'd
    /// - `None` if we're not on a confirm channel
    pub fn basic_publish(&self, exchange: &str, routing_key: &str, payload: Vec<u8>, options: BasicPublishOptions, properties: BasicProperties) -> impl Future<Item = Option<bool>, Error = Error> + Send + 'static {
      let delivery_tag = self.inner.basic_publish(exchange, routing_key, options, payload, properties);
      let transport = self.transport.clone();
      let mut inner = self.inner.clone();

      future::result(delivery_tag.map_err(|e| ErrorKind::ProtocolError("Could not publish".to_string(), e).into())).and_then(move |delivery_tag| {
        if delivery_tag.is_some() {
          trace!("{} returning closure", "basic_publish");
        }

        future::poll_fn(move || {
          let mut transport = transport.lock();

          if let Some(delivery_tag) = delivery_tag {
            Self::wait_for_ack(&mut inner, &mut transport, delivery_tag, move |channel, delivery_tag| {
              if channel.status.confirm() {
                if channel.acknowledgements.is_acked(delivery_tag) {
                  Ok(Async::Ready(Some(true)))
                } else if channel.acknowledgements.is_nacked(delivery_tag) {
                  Ok(Async::Ready(Some(false)))
                } else {
                  debug!("message with tag {} still in unacked", delivery_tag);
                  task::current().notify();
                  Ok(Async::NotReady)
                }
              } else {
                Ok(Async::Ready(None))
              }
            })
          } else {
            transport.poll().map(|r| r.map(|_| None))
          }
        })
      })
    }

    /// creates a consumer stream
    ///
    /// returns a future of a `Consumer` that resolves once the method succeeds
    ///
    /// `Consumer` implements `futures::Stream`, so it can be used with any of
    /// the usual combinators
    pub fn basic_consume(&self, queue: &Queue, consumer_tag: &str, options: BasicConsumeOptions, arguments: FieldTable) -> impl Future<Item = Consumer<T>, Error = Error> + Send + 'static {
        let consumer_tag = consumer_tag.to_string();
        let queue_name = queue.name();
        let mut consumer = Consumer::new(self.transport.clone(), self.id(), queue.name(), consumer_tag.to_owned());
        let subscriber = consumer.subscriber();
        let request_id = self.inner.basic_consume(&queue_name, &consumer_tag, options, arguments, Box::new(subscriber));
        let inner = self.inner.clone();

        self.run_on_locked_transport("basic_consume", "Could not start consumer", request_id).and_then(move |request_id| {
            future::poll_fn(move || {
              if let Some(consumer_tag) = inner.generated_names.get(request_id.expect("expected request_id")) {
                return Ok(Async::Ready(consumer_tag))
              } else {
                task::current().notify();
                return Ok(Async::NotReady)
              }
            })
          }).map(|consumer_tag| {
            trace!("basic_consume received response, returning consumer");
            consumer.update_consumer_tag(consumer_tag);
            consumer
        })
    }

    pub fn basic_cancel(&self, consumer_tag: &str, options: BasicCancelOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.basic_cancel(consumer_tag, options);

        self.run_on_locked_transport("basic_cancel", "Could not cancel consumer", request_id).map(|_| ())
    }

    pub fn basic_recover(&self, options: BasicRecoverOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.basic_recover(options);

        self.run_on_locked_transport("basic_recover", "Could not recover", request_id).map(|_| ())
    }

    pub fn basic_recover_async(&self, options: BasicRecoverAsyncOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.basic_recover_async(options);

        self.run_on_locked_transport("basic_recover_async", "Could not recover", request_id).map(|_| ())
    }

    /// acks a message
    pub fn basic_ack(&self, delivery_tag: u64, multiple: bool) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.basic_ack(delivery_tag, BasicAckOptions { multiple });

        self.run_on_locked_transport("basic_ack", "Could not ack message", request_id).map(|_| ())
    }

    /// nacks a message
    pub fn basic_nack(&self, delivery_tag: u64, multiple: bool, requeue: bool) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.basic_nack(delivery_tag, BasicNackOptions { multiple, requeue });

        self.run_on_locked_transport("basic_nack", "Could not nack message", request_id).map(|_| ())
    }

    /// rejects a message
    pub fn basic_reject(&self, delivery_tag: u64, options: BasicRejectOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.basic_reject(delivery_tag, options);

        self.run_on_locked_transport("basic_reject", "Could not reject message", request_id).map(|_| ())
    }

    /// gets a message
    pub fn basic_get(&self, queue: &str, options: BasicGetOptions) -> impl Future<Item = BasicGetMessage, Error = Error> + Send + 'static {
        let _queue = queue.to_string();
        let receive_transport = self.transport.clone();
        let inner = self.inner.clone();
        let receive_future = future::poll_fn(move || {
            let mut transport = receive_transport.lock();
            transport.poll()?;
            if let Some(message) = inner.queues.next_basic_get_message(&_queue) {
                return Ok(Async::Ready(message));
            }
            Ok(Async::NotReady)
        });
        let request_id = self.inner.basic_get(queue, options);

        self.run_on_locked_transport_full("basic_get", "Could not get message", request_id, |channel, request_id| {
            match channel.requests.was_successful(request_id) {
                Some(answer) => if answer {
                    Ok(Async::Ready(Some(request_id)))
                } else {
                    Err(ErrorKind::EmptyBasicGet.into())
                },
                None         => {
                    task::current().notify();
                    Ok(Async::NotReady)
                }
            }
        }).and_then(|_| receive_future)
    }

    /// Purge a queue.
    ///
    /// This method removes all messages from a queue which are not awaiting acknowledgment.
    pub fn queue_purge(&self, queue_name: &str, options: QueuePurgeOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.queue_purge(queue_name, options);

        self.run_on_locked_transport("queue_purge", "Could not purge queue", request_id).map(|_| ())
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
    pub fn queue_delete(&self, queue_name: &str, options: QueueDeleteOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.queue_delete(queue_name, options);

        self.run_on_locked_transport("queue_purge", "Could not purge queue", request_id).map(|_| ())
    }

    /// closes the channel
    pub fn close(&self, code: u16, message: &str) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.channel_close(code, message, 0, 0);

        self.run_on_locked_transport("close", "Could not close channel", request_id).map(|_| ())
    }

    /// ack a channel close
    pub fn close_ok(&self) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.channel_close_ok();

        self.run_on_locked_transport("close_ok", "Could not ack closed channel", request_id).map(|_| ())
    }

    /// update a channel flow
    pub fn channel_flow(&self, options: ChannelFlowOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.channel_flow(options);

        self.run_on_locked_transport("channel_flow", "Could not update channel flow", request_id).map(|_| ())
    }

    /// ack an update to a channel flow
    pub fn channel_flow_ok(&self, options: ChannelFlowOkOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.channel_flow_ok(options);

        self.run_on_locked_transport("channel_flow_ok", "Could not ack update to channel flow", request_id).map(|_| ())
    }

    pub fn tx_select(&self) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.tx_select();

        self.run_on_locked_transport("tx_select", "Could not start transaction", request_id).map(|_| ())
    }

    pub fn tx_commit(&self) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.tx_commit();

        self.run_on_locked_transport("tx_commit", "Could not commit transaction", request_id).map(|_| ())
    }

    pub fn tx_rollback(&self) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.inner.tx_rollback();

        self.run_on_locked_transport("tx_rollback", "Could not rollback transaction", request_id).map(|_| ())
    }

    fn run_on_locked_transport_full<Finished>(&self, method: &str, error_msg: &str, request_id: RequestResult, finished: Finished) -> impl Future<Item = Option<RequestId>, Error = Error> + Send + 'static
        where Finished: 'static + Send + Fn(&mut InnerChannel, RequestId) -> Poll<Option<RequestId>, Error> {
        trace!("run on locked transport; method={:?}", method);
        let transport = self.transport.clone();
        let method = method.to_string();
        let error_msg = error_msg.to_string();
        let mut inner = self.inner.clone();

        trace!("run on locked transport; method={:?} request_id={:?}", method, request_id);
        future::result(request_id.map_err(|e| ErrorKind::ProtocolError(error_msg.clone(), e).into())).and_then(move |request_id| {
            if request_id.is_some() {
                trace!("{} returning closure", method);
            }

            future::poll_fn(move || {
                let mut transport = transport.lock();

                if let Some(request_id) = request_id {
                    Self::wait_for_answer(&mut inner, &mut transport, request_id, &finished)
                } else {
                    transport.poll().map(|r| r.map(|_| None))
                }
            })
        })
    }

    fn run_on_lock_transport_basic_finished(channel: &mut InnerChannel, request_id: RequestId) -> Poll<Option<RequestId>, Error> {
        match channel.requests.was_successful(request_id) {
            Some(answer) if answer => Ok(Async::Ready(Some(request_id))),
            _                      => {
                task::current().notify();
                Ok(Async::NotReady)
            }
        }
    }

    fn run_on_locked_transport(&self, method: &str, error: &str, request_id: RequestResult) -> impl Future<Item = Option<RequestId>, Error = Error> + Send + 'static {
        self.run_on_locked_transport_full(method, error, request_id, Self::run_on_lock_transport_basic_finished)
    }

    /// internal method to wait until a request succeeds
    pub fn wait_for_answer<Finished>(channel: &mut InnerChannel, tr: &mut AMQPTransport<T>, request_id: RequestId, finished: &Finished) -> Poll<Option<RequestId>, Error>
        where Finished: 'static + Send + Fn(&mut InnerChannel, RequestId) -> Poll<Option<RequestId>, Error> {
            trace!("wait for answer; request_id={:?}", request_id);
            tr.poll()?;
            trace!("wait for answer transport poll; request_id={:?} status=NotReady", request_id);
            if let Async::Ready(r) = finished(channel, request_id)? {
                trace!("wait for answer; request_id={:?} status=Ready result={:?}", request_id, r);
                return Ok(Async::Ready(r));
            }
            trace!("wait for answer; request_id={:?} status=NotReady", request_id);
            task::current().notify();
            Ok(Async::NotReady)
    }

    /// internal method to wait until a request succeeds
    pub fn wait_for_ack<Finished>(channel: &mut InnerChannel, tr: &mut AMQPTransport<T>, delivery_tag: DeliveryTag, finished: Finished) -> Poll<Option<bool>, Error>
      where Finished: 'static + Send + Fn(&mut InnerChannel, DeliveryTag) -> Poll<Option<bool>, Error> {
        trace!("wait for aack; delivery_tag={:?}", delivery_tag);
        tr.poll()?;
        trace!("wait for ack; transport poll; delivery_tag={:?} status=NotReady", delivery_tag);
        if let Async::Ready(t) = finished(channel, delivery_tag)? {
          trace!("wait for ack; delivery_tag={:?} status=Ready result={:?}", delivery_tag, t);
          return Ok(Async::Ready(t));
        }
        trace!("wait for ack; delivery_tag={:?} status=NotReady", delivery_tag);
        task::current().notify();
        Ok(Async::NotReady)
    }
}
