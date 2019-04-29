pub use lapin_async::channel::BasicProperties;
pub use lapin_async::channel::options::*;

use futures::{Async, Future, future, Poll, Stream, task};
use lapin_async;
use lapin_async::api::{ChannelState, RequestId};
use lapin_async::channel::ChannelHandle;
use lapin_async::connection::Connection;
use log::{info, trace};
use parking_lot::Mutex;
use tokio_io::{AsyncRead, AsyncWrite};

use std::sync::Arc;

use crate::consumer::Consumer;
use crate::error::{Error, ErrorKind};
use crate::message::BasicGetMessage;
use crate::queue::Queue;
use crate::transport::*;
use crate::types::*;

pub type RequestResult = Result<Option<RequestId>, lapin_async::error::Error>;

/// `Channel` provides methods to act on a channel, such as managing queues
//#[derive(Clone)]
pub struct Channel<T> {
  pub transport: Arc<Mutex<AMQPTransport<T>>>,
      handle:    ChannelHandle,
}

impl<T> Clone for Channel<T>
    where T: Send {
  fn clone(&self) -> Channel<T> {
    Channel {
      transport: self.transport.clone(),
      handle:    self.handle.clone(),
    }
  }
}

impl<T: AsyncRead+AsyncWrite+Send+Sync+'static> Channel<T> {
    /// create a channel
    pub fn create(transport: Arc<Mutex<AMQPTransport<T>>>) -> impl Future<Item = Self, Error = Error> + Send + 'static {
        let channel_transport = transport.clone();

        future::poll_fn(move || {
            let mut transport = channel_transport.lock();
            if let Some(handle) = transport.conn.create_channel() {
                return Ok(Async::Ready(Channel {
                    handle,
                    transport: channel_transport.clone(),
                }))
            } else {
                return Err(ErrorKind::ChannelLimitReached.into());
            }
        }).and_then(|mut channel| {
            let request_id = channel.handle.channel_open("");
            let channel_id = channel.handle.id;
            channel.run_on_locked_transport("create", "Could not create channel", request_id).and_then(move |_| {
                future::poll_fn(move || {
                    let transport = transport.lock();

                    match transport.conn.get_state(channel_id) {
                        Some(ChannelState::Connected) => Ok(Async::Ready(())),
                        Some(ChannelState::Error) | Some(ChannelState::Closed) => {
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
      self.handle.id
    }

    /// request access
    ///
    /// returns a future that resolves once the access is granted
    pub fn access_request(&mut self, realm: &str, options: AccessRequestOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.access_request(realm, options);

        self.run_on_locked_transport("access_request", "Could not request access", request_id).map(|_| ())
    }

    /// declares an exchange
    ///
    /// returns a future that resolves once the exchange is available
    pub fn exchange_declare(&mut self, name: &str, exchange_type: &str, options: ExchangeDeclareOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.exchange_declare(name, exchange_type, options, arguments);

        self.run_on_locked_transport("exchange_declare", "Could not declare exchange", request_id).map(|_| ())
    }

    /// deletes an exchange
    ///
    /// returns a future that resolves once the exchange is deleted
    pub fn exchange_delete(&mut self, name: &str, options: ExchangeDeleteOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.exchange_delete(name, options);

        self.run_on_locked_transport("exchange_delete", "Could not delete exchange", request_id).map(|_| ())
    }

    /// binds an exchange to another exchange
    ///
    /// returns a future that resolves once the exchanges are bound
    pub fn exchange_bind(&mut self, destination: &str, source: &str, routing_key: &str, options: ExchangeBindOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.exchange_bind(destination, source, routing_key, options, arguments);

        self.run_on_locked_transport("exchange_bind", "Could not bind exchange", request_id).map(|_| ())
    }

    /// unbinds an exchange from another one
    ///
    /// returns a future that resolves once the exchanges are unbound
    pub fn exchange_unbind(&mut self, destination: &str, source: &str, routing_key: &str, options: ExchangeUnbindOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.exchange_unbind(destination, source, routing_key, options, arguments);

        self.run_on_locked_transport("exchange_unbind", "Could not unbind exchange", request_id).map(|_| ())
    }

    /// declares a queue
    ///
    /// returns a future that resolves once the queue is available
    ///
    /// the `mandatory` and `ìmmediate` options can be set to true,
    /// but the return message will not be handled
    pub fn queue_declare(&mut self, name: &str, options: QueueDeclareOptions, arguments: FieldTable) -> impl Future<Item = Queue, Error = Error> + Send + 'static {
        let request_id = self.handle.queue_declare(name, options, arguments);
        let transport = self.transport.clone();
        let handle = self.handle.clone();

        self.run_on_locked_transport("queue_declare", "Could not declare queue", request_id).and_then(move |request_id| {
            future::poll_fn(move || {
              let mut transport = transport.lock();
              if let Some(queue) = transport.conn.get_generated_name(request_id.expect("expected request_id")) {
                let (consumer_count, message_count) = handle.get_queue_stats(&queue);
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
    pub fn queue_bind(&mut self, name: &str, exchange: &str, routing_key: &str, options: QueueBindOptions, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.queue_bind(name, exchange, routing_key, options, arguments);

        self.run_on_locked_transport("queue_bind", "Could not bind queue", request_id).map(|_| ())
    }

    /// unbinds a queue from the exchange
    ///
    /// returns a future that resolves once the queue is unbound from the exchange
    pub fn queue_unbind(&mut self, name: &str, exchange: &str, routing_key: &str, arguments: FieldTable) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.queue_unbind(name, exchange, routing_key, arguments);

        self.run_on_locked_transport("queue_unbind", "Could not unbind queue from the exchange", request_id).map(|_| ())
    }

    /// sets up confirm extension for this channel
    pub fn confirm_select(&mut self, options: ConfirmSelectOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.confirm_select(options);

        self.run_on_locked_transport("confirm_select", "Could not activate confirm extension", request_id).map(|_| ())
    }

    /// specifies quality of service for a channel
    pub fn basic_qos(&mut self, prefetch_count: ShortUInt, options: BasicQosOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.basic_qos(prefetch_count, options);

        self.run_on_locked_transport("basic_qos", "Could not setup qos", request_id).map(|_| ())
    }

    /// publishes a message on a queue
    ///
    /// the future's result is:
    /// - `Some(request_id)` if we're on a confirm channel and the message was ack'd
    /// - `None` if we're not on a confirm channel or the message was nack'd
    pub fn basic_publish(&mut self, exchange: &str, routing_key: &str, payload: Vec<u8>, options: BasicPublishOptions, properties: BasicProperties) -> impl Future<Item = Option<RequestId>, Error = Error> + Send + 'static {
        let channel_id = self.handle.id;
        let request_id = self.handle.basic_publish(exchange, routing_key, options, payload, properties);

        self.run_on_locked_transport_full("basic_publish", "Could not publish", request_id, move |conn, delivery_tag| {
            conn.channels.get_mut(&channel_id).and_then(|c| {
                if c.confirm() {
                    if c.acked.remove(&delivery_tag) {
                        Some(Ok(Async::Ready(Some(delivery_tag))))
                    } else if c.nacked.remove(&delivery_tag) {
                        Some(Ok(Async::Ready(None)))
                    } else {
                        info!("message with tag {} still in unacked", delivery_tag);
                        task::current().notify();
                        Some(Ok(Async::NotReady))
                    }
                } else {
                    None
                }
            }).unwrap_or(Ok(Async::Ready(None)))
        })
    }

    /// creates a consumer stream
    ///
    /// returns a future of a `Consumer` that resolves once the method succeeds
    ///
    /// `Consumer` implements `futures::Stream`, so it can be used with any of
    /// the usual combinators
    pub fn basic_consume(&mut self, queue: &Queue, consumer_tag: &str, options: BasicConsumeOptions, arguments: FieldTable) -> impl Future<Item = Consumer<T>, Error = Error> + Send + 'static {
        let transport = self.transport.clone();
        let consumer_tag = consumer_tag.to_string();
        let queue_name = queue.name();
        let mut consumer = Consumer::new(self.transport.clone(), self.handle.id, queue.name(), consumer_tag.to_owned());
        let subscriber = consumer.subscriber();
        let request_id = self.handle.basic_consume(&queue_name, &consumer_tag, options, arguments, Box::new(subscriber));

        self.run_on_locked_transport("basic_consume", "Could not start consumer", request_id).and_then(move |request_id| {
            future::poll_fn(move || {
              let mut transport = transport.lock();
              if let Some(consumer_tag) = transport.conn.get_generated_name(request_id.expect("expected request_id")) {
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

    /// acks a message
    pub fn basic_ack(&mut self, delivery_tag: u64, multiple: bool) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.basic_ack(delivery_tag, BasicAckOptions { multiple });

        self.run_on_locked_transport("basic_ack", "Could not ack message", request_id).map(|_| ())
    }

    /// nacks a message
    pub fn basic_nack(&mut self, delivery_tag: u64, multiple: bool, requeue: bool) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.basic_nack(delivery_tag, BasicNackOptions { multiple, requeue });

        self.run_on_locked_transport("basic_nack", "Could not nack message", request_id).map(|_| ())
    }

    /// rejects a message
    pub fn basic_reject(&mut self, delivery_tag: u64, options: BasicRejectOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.basic_reject(delivery_tag, options);

        self.run_on_locked_transport("basic_reject", "Could not reject message", request_id).map(|_| ())
    }

    /// gets a message
    pub fn basic_get(&mut self, queue: &str, options: BasicGetOptions) -> impl Future<Item = BasicGetMessage, Error = Error> + Send + 'static {
        let channel_id = self.handle.id;
        let _queue = queue.to_string();
        let receive_transport = self.transport.clone();
        let receive_future = future::poll_fn(move || {
            let mut transport = receive_transport.lock();
            transport.poll()?;
            if let Some(message) = transport.conn.next_basic_get_message(channel_id, &_queue) {
                return Ok(Async::Ready(message));
            }
            Ok(Async::NotReady)
        });
        let request_id = self.handle.basic_get(queue, options);

        self.run_on_locked_transport_full("basic_get", "Could not get message", request_id, |conn, request_id| {
            match conn.finished_get_result(request_id) {
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
    pub fn queue_purge(&mut self, queue_name: &str, options: QueuePurgeOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.queue_purge(queue_name, options);

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
    pub fn queue_delete(&mut self, queue_name: &str, options: QueueDeleteOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.queue_delete(queue_name, options);

        self.run_on_locked_transport("queue_purge", "Could not purge queue", request_id).map(|_| ())
    }

    /// closes the channel
    pub fn close(&mut self, code: u16, message: &str) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.channel_close(code, message, 0, 0);

        self.run_on_locked_transport("close", "Could not close channel", request_id).map(|_| ())
    }

    /// ack a channel close
    pub fn close_ok(&mut self) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.channel_close_ok();

        self.run_on_locked_transport("close_ok", "Could not ack closed channel", request_id).map(|_| ())
    }

    /// update a channel flow
    pub fn channel_flow(&mut self, options: ChannelFlowOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.channel_flow(options);

        self.run_on_locked_transport("channel_flow", "Could not update channel flow", request_id).map(|_| ())
    }

    /// ack an update to a channel flow
    pub fn channel_flow_ok(&mut self, options: ChannelFlowOkOptions) -> impl Future<Item = (), Error = Error> + Send + 'static {
        let request_id = self.handle.channel_flow_ok(options);

        self.run_on_locked_transport("channel_flow_ok", "Could not ack update to channel flow", request_id).map(|_| ())
    }

    fn run_on_locked_transport_full<Finished>(&self, method: &str, error_msg: &str, request_id: RequestResult, finished: Finished) -> impl Future<Item = Option<RequestId>, Error = Error> + Send + 'static
        where Finished: 'static + Send + Fn(&mut Connection, RequestId) -> Poll<Option<RequestId>, Error> {
        trace!("run on locked transport; method={:?}", method);
        let transport = self.transport.clone();
        let method = method.to_string();
        let error_msg = error_msg.to_string();

        trace!("run on locked transport; method={:?} request_id={:?}", method, request_id);
        future::result(request_id.map_err(|e| ErrorKind::ProtocolError(error_msg.clone(), e).into())).and_then(move |request_id| {
            if request_id.is_some() {
                trace!("{} returning closure", method);
            }

            future::poll_fn(move || {
                let mut transport = transport.lock();

                if let Some(request_id) = request_id {
                    Self::wait_for_answer(&mut transport, request_id, &finished)
                } else {
                    transport.poll().map(|r| r.map(|_| None))
                }
            })
        })
    }

    fn run_on_lock_transport_basic_finished(conn: &mut Connection, request_id: RequestId) -> Poll<Option<RequestId>, Error> {
        match conn.has_finished(request_id) {
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
    pub fn wait_for_answer<Finished>(tr: &mut AMQPTransport<T>, request_id: RequestId, finished: &Finished) -> Poll<Option<RequestId>, Error>
        where Finished: 'static + Send + Fn(&mut Connection, RequestId) -> Poll<Option<RequestId>, Error> {
            trace!("wait for answer; request_id={:?}", request_id);
            tr.poll()?;
            trace!("wait for answer transport poll; request_id={:?} status=NotReady", request_id);
            if let Async::Ready(r) = finished(&mut tr.conn, request_id)? {
                trace!("wait for answer; request_id={:?} status=Ready result={:?}", request_id, r);
                return Ok(Async::Ready(r));
            }
            trace!("wait for answer; request_id={:?} status=NotReady", request_id);
            task::current().notify();
            Ok(Async::NotReady)
    }
}
