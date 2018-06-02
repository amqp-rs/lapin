use std::io::{self,Error,ErrorKind};
use futures::{Async,Future,future,Poll,task};
use tokio_io::{AsyncRead,AsyncWrite};
use std::sync::{Arc,Mutex};
use lapin_async;
use lapin_async::api::{ChannelState, RequestId};
use lapin_async::connection::Connection;
use lapin_async::generated::basic;

use transport::*;
use message::BasicGetMessage;
use types::FieldTable;
use consumer::Consumer;
use queue::Queue;

/// `Channel` provides methods to act on a channel, such as managing queues
//#[derive(Clone)]
pub struct Channel<T> {
  pub transport: Arc<Mutex<AMQPTransport<T>>>,
  pub id:    u16,
}

impl<T> Clone for Channel<T>
    where T: Send {
  fn clone(&self) -> Channel<T> {
    Channel {
      transport: self.transport.clone(),
      id:        self.id,
    }
  }
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct AccessRequestOptions {
  pub exclusive: bool,
  pub passive:   bool,
  pub active:    bool,
  pub write:     bool,
  pub read:      bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct ExchangeDeclareOptions {
  pub ticket:      u16,
  pub passive:     bool,
  pub durable:     bool,
  pub auto_delete: bool,
  pub internal:    bool,
  pub nowait:      bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct ExchangeDeleteOptions {
  pub ticket:    u16,
  pub if_unused: bool,
  pub nowait:    bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct ExchangeBindOptions {
  pub ticket: u16,
  pub nowait: bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct ExchangeUnbindOptions {
  pub ticket: u16,
  pub nowait: bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct QueueDeclareOptions {
  pub ticket:      u16,
  pub passive:     bool,
  pub durable:     bool,
  pub exclusive:   bool,
  pub auto_delete: bool,
  pub nowait:      bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct QueueUnbindOptions {
  pub ticket: u16
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct ConfirmSelectOptions {
  pub nowait: bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct QueueBindOptions {
  pub ticket: u16,
  pub nowait: bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct QueuePurgeOptions {
  pub ticket: u16,
  pub nowait: bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct BasicPublishOptions {
  pub ticket:    u16,
  pub mandatory: bool,
  pub immediate: bool,
}

pub type BasicProperties = basic::Properties;

#[derive(Clone,Debug,Default,PartialEq)]
pub struct BasicConsumeOptions {
  pub ticket:    u16,
  pub no_local:  bool,
  pub no_ack:    bool,
  pub exclusive: bool,
  pub no_wait:   bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct BasicGetOptions {
  pub ticket:    u16,
  pub no_ack:    bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct BasicQosOptions {
  pub prefetch_size:  u32,
  pub prefetch_count: u16,
  pub global:         bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct QueueDeleteOptions {
  pub ticket:    u16,
  pub if_unused: bool,
  pub if_empty:  bool,
  pub no_wait:   bool,
}

#[derive(Clone,Debug,Default,PartialEq)]
pub struct ChannelFlowOptions {
  pub active: bool,
}

impl<T: AsyncRead+AsyncWrite+Send+'static> Channel<T> {
    /// create a channel
    pub fn create(transport: Arc<Mutex<AMQPTransport<T>>>) -> Box<Future<Item = Self, Error = io::Error> + Send + 'static> {
        let channel_transport = transport.clone();
        let create_channel = future::poll_fn(move || {
            let mut transport = lock_transport!(channel_transport);
            if let Some(id) = transport.conn.create_channel() {
                return Ok(Async::Ready(Channel {
                    id,
                    transport: channel_transport.clone(),
                }))
            } else {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "The maximum number of channels for this connection has been reached"));
            }
        });

        Box::new(create_channel.and_then(|channel| {
            let channel_id = channel.id;
            channel.run_on_locked_transport("create", "Could not create channel", move |transport| {
                transport.conn.channel_open(channel_id, "".to_string()).map(Some)
            }).and_then(move |_| {
                future::poll_fn(move || {
                    let transport = lock_transport!(transport);

                    match transport.conn.get_state(channel_id) {
                        Some(ChannelState::Connected) => return Ok(Async::Ready(())),
                        Some(ChannelState::Error)     => return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open channel"))),
                        Some(ChannelState::Closed)    => return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open channel"))),
                        _                             => {
                            task::current().notify();
                            return Ok(Async::NotReady);
                        }
                    }
                })
            }).map(move |_| {
                channel
            })
        }))
    }

    /// request access
    ///
    /// returns a future that resolves once the access is granted
    pub fn access_request(&self, realm: &str, options: AccessRequestOptions) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let realm = realm.to_string();

        Box::new(self.run_on_locked_transport("access_request", "Could not request access", move |transport| {
            transport.conn.access_request(channel_id, realm,
                options.exclusive, options.passive, options.active, options.write, options.read).map(Some)
        }).map(|_| ()))
    }

    /// declares an exchange
    ///
    /// returns a future that resolves once the exchange is available
    pub fn exchange_declare(&self, name: &str, exchange_type: &str, options: ExchangeDeclareOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let name = name.to_string();
        let exchange_type = exchange_type.to_string();

        Box::new(self.run_on_locked_transport("exchange_declare", "Could not declare exchange", move |transport| {
            transport.conn.exchange_declare(channel_id, options.ticket, name, exchange_type,
                options.passive, options.durable, options.auto_delete, options.internal, options.nowait, arguments).map(Some)
        }).map(|_| ()))
    }

    /// deletes an exchange
    ///
    /// returns a future that resolves once the exchange is deleted
    pub fn exchange_delete(&self, name: &str, options: ExchangeDeleteOptions) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let name = name.to_string();

        Box::new(self.run_on_locked_transport("exchange_delete", "Could not delete exchange", move |transport| {
            transport.conn.exchange_delete(channel_id, options.ticket, name,
                options.if_unused, options.nowait).map(Some)
        }).map(|_| ()))
    }

    /// binds an exchange to another exchange
    ///
    /// returns a future that resolves once the exchanges are bound
    pub fn exchange_bind(&self, destination: &str, source: &str, routing_key: &str, options: ExchangeBindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let destination = destination.to_string();
        let source = source.to_string();
        let routing_key = routing_key.to_string();

        Box::new(self.run_on_locked_transport("exchange_bind", "Could not bind exchange", move |transport| {
            transport.conn.exchange_bind(channel_id, options.ticket, destination, source, routing_key,
                options.nowait, arguments).map(Some)
        }).map(|_| ()))
    }

    /// unbinds an exchange from another one
    ///
    /// returns a future that resolves once the exchanges are unbound
    pub fn exchange_unbind(&self, destination: &str, source: &str, routing_key: &str, options: ExchangeUnbindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let destination = destination.to_string();
        let source = source.to_string();
        let routing_key = routing_key.to_string();

        Box::new(self.run_on_locked_transport("exchange_unbind", "Could not unbind exchange", move |transport| {
            transport.conn.exchange_unbind(channel_id, options.ticket, destination, source, routing_key,
                options.nowait, arguments).map(Some)
        }).map(|_| ()))
    }

    /// declares a queue
    ///
    /// returns a future that resolves once the queue is available
    ///
    /// the `mandatory` and `ìmmediate` options can be set to true,
    /// but the return message will not be handled
    pub fn queue_declare(&self, name: &str, options: QueueDeclareOptions, arguments: FieldTable) -> Box<Future<Item = Queue, Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let name = name.to_string();
        let transport = self.transport.clone();

        Box::new(self.run_on_locked_transport("queue_declare", "Could not declare queue", move |transport| {
            transport.conn.queue_declare(channel_id, options.ticket, name,
                options.passive, options.durable, options.exclusive, options.auto_delete, options.nowait, arguments).map(Some)
          }).and_then(|request_id| {
            future::poll_fn(move || {
              let mut transport = lock_transport!(transport);
              if let Some(queue) = transport.conn.get_generated_name(request_id.expect("expected request_id")) {
                return Ok(Async::Ready(queue))
              } else {
                task::current().notify();
                return Ok(Async::NotReady)
              }
            })
        }).map(Queue::new))
    }

    /// binds a queue to an exchange
    ///
    /// returns a future that resolves once the queue is bound to the exchange
    pub fn queue_bind(&self, name: &str, exchange: &str, routing_key: &str, options: QueueBindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let name = name.to_string();
        let exchange = exchange.to_string();
        let routing_key = routing_key.to_string();

        Box::new(self.run_on_locked_transport("queue_bind", "Could not bind queue", move |transport| {
            transport.conn.queue_bind(channel_id, options.ticket, name, exchange, routing_key,
                options.nowait, arguments).map(Some)
        }).map(|_| ()))
    }

    /// unbinds a queue from the exchange
    ///
    /// returns a future that resolves once the queue is unbound from the exchange
    pub fn queue_unbind(&self, name: &str, exchange: &str, routing_key: &str, options: QueueUnbindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let name = name.to_string();
        let exchange = exchange.to_string();
        let routing_key = routing_key.to_string();

        Box::new(self.run_on_locked_transport("queue_unbind", "Could not unbind queue from the exchange", move |transport| {
            transport.conn.queue_unbind(channel_id, options.ticket, name, exchange, routing_key, arguments).map(Some)
        }).map(|_| ()))
    }

    /// sets up confirm extension for this channel
    pub fn confirm_select(&self, options: ConfirmSelectOptions) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("confirm_select", "Could not activate confirm extension", move |transport| {
            transport.conn.confirm_select(channel_id, options.nowait).map(Some)
        }).map(|_| ()))
    }

    /// specifies quality of service for a channel
    pub fn basic_qos(&self, options: BasicQosOptions) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("basic_qos", "Could not setup qos", move |transport| {
            transport.conn.basic_qos(channel_id, options.prefetch_size, options.prefetch_count, options.global).map(|_| None)
        }).map(|_| ()))
    }

    /// publishes a message on a queue
    ///
    /// the future's result is:
    /// - `Some(request_id)` if we're on a confirm channel and the message was ack'd
    /// - `None` if we're not on a confirm channel or the message was nack'd
    pub fn basic_publish(&self, exchange: &str, routing_key: &str, payload: &[u8], options: BasicPublishOptions, properties: BasicProperties) -> Box<Future<Item = Option<RequestId>, Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let exchange = exchange.to_string();
        let routing_key = routing_key.to_string();

        Box::new(self.run_on_locked_transport_full("basic_publish", "Could not publish", move |transport| {
            transport.conn.basic_publish(channel_id, options.ticket, exchange, routing_key,
                options.mandatory, options.immediate).map(Some)
        }, move |conn, delivery_tag| {
            conn.channels.get_mut(&channel_id).and_then(|c| {
                if c.confirm {
                    if c.acked.remove(&delivery_tag) {
                        Some(Ok(Async::Ready(Some(delivery_tag))))
                    } else if c.nacked.remove(&delivery_tag) {
                        Some(Ok(Async::Ready(None)))
                    } else {
                        info!("message with tag {} still in unacked: {:?}", delivery_tag, c.unacked);
                        task::current().notify();
                        Some(Ok(Async::NotReady))
                    }
                } else {
                    None
                }
            }).unwrap_or(Ok(Async::Ready(None)))
        }, Some((channel_id, payload, properties))))
    }

    /// creates a consumer stream
    ///
    /// returns a future of a `Consumer` that resolves once the method succeeds
    ///
    /// `Consumer` implements `futures::Stream`, so it can be used with any of
    /// the usual combinators
    pub fn basic_consume(&self, queue: &Queue, consumer_tag: &str, options: BasicConsumeOptions, arguments: FieldTable) -> Box<Future<Item = Consumer<T>, Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let consumer_tag = consumer_tag.to_string();
        let transport = self.transport.clone();
        let queue_name = queue.name();
        let mut consumer = Consumer {
            transport:    self.transport.clone(),
            channel_id:   self.id,
            queue:        queue.name(),
            consumer_tag: consumer_tag.to_string(),
            registered:   false,
        };

        Box::new(self.run_on_locked_transport("basic_consume", "Could not start consumer", move |transport| {
            transport.conn.basic_consume(channel_id, options.ticket, queue_name, consumer_tag,
            options.no_local, options.no_ack, options.exclusive, options.no_wait, arguments).map(Some)
          }).and_then(move |request_id| {
            future::poll_fn(move || {
              let mut transport = lock_transport!(transport);
              if let Some(consumer_tag) = transport.conn.get_generated_name(request_id.expect("expected request_id")) {
                return Ok(Async::Ready(consumer_tag))
              } else {
                task::current().notify();
                return Ok(Async::NotReady)
              }
            })
          }).map(|consumer_tag| {
            trace!("basic_consume received response, returning consumer");
            consumer.consumer_tag = consumer_tag;
            consumer
        }))
    }

    /// acks a message
    pub fn basic_ack(&self, delivery_tag: u64) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("basic_ack", "Could not ack message", move |transport| {
            transport.conn.basic_ack(channel_id, delivery_tag, false).map(|_| None)
        }).map(|_| ()))
    }

    /// nacks a message
    pub fn basic_nack(&self, delivery_tag: u64, requeue: bool) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("basic_nack", "Could not nack message", move |transport| {
            transport.conn.basic_nack(channel_id, delivery_tag, false, requeue).map(|_| None)
        }).map(|_| ()))
    }

    /// rejects a message
    pub fn basic_reject(&self, delivery_tag: u64, requeue: bool) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("basic_reject", "Could not reject message", move |transport| {
            transport.conn.basic_reject(channel_id, delivery_tag, requeue).map(|_| None)
        }).map(|_| ()))
    }

    /// gets a message
    pub fn basic_get(&self, queue: &str, options: BasicGetOptions) -> Box<Future<Item = BasicGetMessage, Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let queue = queue.to_string();
        let _queue = queue.to_string();
        let receive_transport = self.transport.clone();
        let receive_future = future::poll_fn(move || {
            let mut transport = lock_transport!(receive_transport);
            if let Async::Ready(_) = transport.poll()? {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "The connection was closed by the remote peer"));
            }
            if let Some(message) = transport.conn.next_basic_get_message(channel_id, &_queue) {
                return Ok(Async::Ready(message));
            }
            Ok(Async::NotReady)
        });

        Box::new(self.run_on_locked_transport_full("basic_get", "Could not get message", move |transport| {
            transport.conn.basic_get(channel_id, options.ticket, queue, options.no_ack).map(Some)
        }, |conn, request_id| {
            match conn.finished_get_result(request_id) {
                Some(answer) => if answer {
                    Ok(Async::Ready(Some(request_id)))
                } else {
                    Err(Error::new(ErrorKind::Other, "basic get returned empty"))
                },
                None         => {
                    task::current().notify();
                    Ok(Async::NotReady)
                }
            }
        }, None).and_then(|_| receive_future))
    }

    /// Purge a queue.
    ///
    /// This method removes all messages from a queue which are not awaiting acknowledgment.
    pub fn queue_purge(&self, queue_name: &str, options: QueuePurgeOptions) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let queue_name = queue_name.to_string();

        Box::new(self.run_on_locked_transport("queue_purge", "Could not purge queue", move |transport| {
            transport.conn.queue_purge(channel_id, options.ticket, queue_name, options.nowait).map(Some)
        }).map(|_| ()))
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
    pub fn queue_delete(&self, queue_name: &str, options: QueueDeleteOptions) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let queue_name = queue_name.to_string();

        Box::new(self.run_on_locked_transport("queue_purge", "Could not purge queue", move |transport| {
            transport.conn.queue_delete(channel_id, options.ticket, queue_name, options.if_unused, options.if_empty, options.no_wait).map(Some)
        }).map(|_| ()))
    }

    /// closes the channel
    pub fn close(&self, code: u16, message: &str) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;
        let message = message.to_string();

        Box::new(self.run_on_locked_transport("close", "Could not close channel", move |transport| {
            transport.conn.channel_close(channel_id, code, message, 0, 0).map(|_| None)
        }).map(|_| ()))
    }

    /// ack a channel close
    pub fn close_ok(&self) -> Box<Future<Item = (), Error = io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("close_ok", "Could not ack closed channel", move |transport| {
            transport.conn.channel_close_ok(channel_id).map(|_| None)
        }).map(|_| ()))
    }

    /// update a channel flow
    pub fn channel_flow(&self, options: ChannelFlowOptions) -> Box<Future<Item = (), Error=io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("channel_flow", "Could not update channel flow", move |transport| {
            transport.conn.channel_flow(channel_id, options.active).map(|_| None)
        }).map(|_| ()))
    }

    /// ack an update to a channel flow
    pub fn channel_flow_ok(&self, options: ChannelFlowOptions) -> Box<Future<Item = (), Error=io::Error> + Send + 'static> {
        let channel_id = self.id;

        Box::new(self.run_on_locked_transport("channel_flow_ok", "Could not ack update to channel flow", move |transport| {
            transport.conn.channel_flow_ok(channel_id, options.active).map(|_| None)
        }).map(|_| ()))
    }

    fn run_on_locked_transport_full<Action, Finished>(&self, method: &str, error: &str, action: Action, finished: Finished, payload: Option<(u16, &[u8], BasicProperties)>) -> Box<Future<Item = Option<RequestId>, Error = io::Error> + Send + 'static>
        where Action:   'static + Send + FnOnce(&mut AMQPTransport<T>) -> Result<Option<RequestId>, lapin_async::error::Error>,
              Finished: 'static + Send + Fn(&mut Connection, RequestId) -> Poll<Option<RequestId>, io::Error> {
        trace!("run on locked transport; method={:?}", method);
        if let Ok(mut transport) = self.transport.lock() {
            match action(&mut transport) {
                Err(e)         => Box::new(future::err(Error::new(ErrorKind::Other, format!("{}: {:?}", error, e)))),
                Ok(request_id) => {
                    trace!("run on locked transport; method={:?} request_id={:?}", method, request_id);

                    if let Some((channel_id, payload, properties)) = payload {
                        transport.send_content_frames(channel_id, payload, properties);
                    }

                    let transport = self.transport.clone();
                    if let Some(request_id) = request_id {
                        trace!("{} returning closure", method);
                        Box::new(Self::wait_for_answer(transport, request_id, finished))
                    } else {
                        Box::new(future::poll_fn(move || {
                            let mut transport = lock_transport!(transport);
                            transport.poll_send()
                        }).map(|_| None))
                    }
                },
            }
        } else {
            //FIXME: if we're there, it means the mutex failed
            Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, "Failed to acquire AMQPTransport mutex")))
        }
    }

    fn run_on_locked_transport<Action>(&self, method: &str, error: &str, action: Action) -> Box<Future<Item = Option<RequestId>, Error = io::Error> + Send + 'static>
        where Action: 'static + Send + FnOnce(&mut AMQPTransport<T>) -> Result<Option<RequestId>, lapin_async::error::Error> {
        Box::new(self.run_on_locked_transport_full(method, error, action, |conn, request_id| {
            match conn.is_finished(request_id) {
                Some(answer) if answer => Ok(Async::Ready(Some(request_id))),
                _                      => {
                    task::current().notify();
                    Ok(Async::NotReady)
                }
            }
        }, None))
    }

    /// internal method to wait until a request succeeds
    pub fn wait_for_answer<Finished>(transport: Arc<Mutex<AMQPTransport<T>>>, request_id: RequestId, finished: Finished) -> Box<Future<Item = Option<RequestId>, Error = io::Error> + Send + 'static>
        where Finished: 'static + Send + Fn(&mut Connection, RequestId) -> Poll<Option<RequestId>, io::Error> {
        Box::new(future::poll_fn(move || {
            trace!("wait for answer; request_id={:?}", request_id);
            let mut tr = lock_transport!(transport);
            if let Async::Ready(_) = tr.poll()? {
                trace!("wait for answer transport poll; request_id={:?} status=Ready", request_id);
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "The connection was closed by the remote peer"));
            }
            trace!("wait for answer transport poll; request_id={:?} status=NotReady", request_id);
            if let Async::Ready(r) = finished(&mut tr.conn, request_id)? {
                trace!("wait for answer; request_id={:?} status=Ready result={:?}", request_id, r);
                return Ok(Async::Ready(r));
            }
            trace!("wait for answer; request_id={:?} status=NotReady", request_id);
            task::current().notify();
            Ok(Async::NotReady)
        }))
    }
}
