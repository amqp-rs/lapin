use std::io::{self,Error,ErrorKind};
use futures::{Async,Future,future,Poll,Stream};
use tokio_io::{AsyncRead,AsyncWrite};
use std::sync::{Arc,Mutex};
use lapin_async;
use lapin_async::api::RequestId;
use lapin_async::connection::Connection;
use lapin_async::queue::Message;
use lapin_async::generated::basic;

use transport::*;
use types::FieldTable;
use consumer::Consumer;

/// `Channel` provides methods to act on a channel, such as managing queues
//#[derive(Clone)]
pub struct Channel<T> {
  pub transport: Arc<Mutex<AMQPTransport<T>>>,
  pub id:    u16,
}

impl<T> Clone for Channel<T> {
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
pub struct QueueDeleteOptions {
  pub ticket:    u16,
  pub if_unused: bool,
  pub if_empty:  bool,
  pub no_wait:   bool,
}

impl<T: AsyncRead+AsyncWrite+'static> Channel<T> {
    /// request access
    ///
    /// returns a future that resolves once the access is granted
    pub fn access_request(&self, realm: &str, options: &AccessRequestOptions) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("access_request", "Could not request access", |mut transport| {
            transport.conn.access_request(self.id, realm.to_string(),
                options.exclusive, options.passive, options.active, options.write, options.read).map(Some)
        })
    }

    /// declares an exchange
    ///
    /// returns a future that resolves once the exchange is available
    pub fn exchange_declare(&self, name: &str, exchange_type: &str, options: &ExchangeDeclareOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("exchange_declare", "Could not declare exchange", |mut transport| {
            transport.conn.exchange_declare(self.id, options.ticket, name.to_string(), exchange_type.to_string(),
                options.passive, options.durable, options.auto_delete, options.internal, options.nowait, arguments.clone()).map(Some)
        })
    }

    /// deletes an exchange
    ///
    /// returns a future that resolves once the exchange is deleted
    pub fn exchange_delete(&self, name: &str, options: &ExchangeDeleteOptions) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("exchange_delete", "Could not delete exchange", |mut transport| {
            transport.conn.exchange_delete(self.id, options.ticket, name.to_string(),
                options.if_unused, options.nowait).map(Some)
        })
    }

    /// binds an exchange to another exchange
    ///
    /// returns a future that resolves once the exchanges are bound
    pub fn exchange_bind(&self, destination: &str, source: &str, routing_key: &str, options: &ExchangeBindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("exchange_bind", "Could not bind exchange", |mut transport| {
            transport.conn.exchange_bind(self.id, options.ticket, destination.to_string(), source.to_string(), routing_key.to_string(),
                options.nowait, arguments.clone()).map(Some)
        })
    }

    /// unbinds an exchange from another one
    ///
    /// returns a future that resolves once the exchanges are unbound
    pub fn exchange_unbind(&self, destination: &str, source: &str, routing_key: &str, options: &ExchangeUnbindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("exchange_unbind", "Could not unbind exchange", |mut transport| {
            transport.conn.exchange_unbind(self.id, options.ticket, destination.to_string(), source.to_string(), routing_key.to_string(),
                options.nowait, arguments.clone()).map(Some)
        })
    }

    /// declares a queue
    ///
    /// returns a future that resolves once the queue is available
    ///
    /// the `mandatory` and `ìmmediate` options can be set to true,
    /// but the return message will not be handled
    pub fn queue_declare(&self, name: &str, options: &QueueDeclareOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("queue_declare", "Could not declare queue", |mut transport| {
            transport.conn.queue_declare(self.id, options.ticket, name.to_string(),
                options.passive, options.durable, options.exclusive, options.auto_delete, options.nowait, arguments.clone()).map(Some)
        })
    }

    /// binds a queue to an exchange
    ///
    /// returns a future that resolves once the queue is bound to the exchange
    pub fn queue_bind(&self, name: &str, exchange: &str, routing_key: &str, options: &QueueBindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("queue_bind", "Could not bind queue", |mut transport| {
            transport.conn.queue_bind(self.id, options.ticket, name.to_string(), exchange.to_string(), routing_key.to_string(),
                options.nowait, arguments.clone()).map(Some)
        })
    }

    /// sets up confirm extension for this channel
    pub fn confirm_select(&self, options: &ConfirmSelectOptions) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("confirm_select", "Could not activate confirm extension", |mut transport| {
            transport.conn.confirm_select(self.id, options.nowait).map(Some)
        })
    }

    /// publishes a message on a queue
    ///
    /// the future's result is:
    /// - `Some(true)` if we're on a confirm channel and the message was ack'd
    /// - `Some(false)` if we're on a confirm channel and the message was nack'd
    /// - `None` if we're not on a confirm channel
    pub fn basic_publish(&self, exchange: &str, queue: &str, payload: &[u8], options: &BasicPublishOptions, properties: BasicProperties) -> Box<Future<Item = Option<bool>, Error = io::Error>> {
        let cl_transport = self.transport.clone();

        if let Ok(mut transport) = self.transport.lock() {
            //FIXME: does not handle the return messages with mandatory and immediate
            match transport.conn.basic_publish(self.id, options.ticket, exchange.to_string(),
            queue.to_string(), options.mandatory, options.immediate) {
                Err(e) => Box::new(
                    future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish: {:?}", e)))
                    ),
                Ok(delivery_tag) => {
                    //TODO: process_frames
                    if let Err(e) = transport.send_and_handle_frames() {
                        let err = format!("Failed to handle frames: {:?}", e);
                        trace!("{}", err);
                        return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
                    }
                    transport.conn.send_content_frames(self.id, 60, payload, properties);
                    if let Err(e) = transport.send_and_handle_frames() {
                        let err = format!("Failed to handle frames: {:?}", e);
                        trace!("{}", err);
                        return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
                    }

                    let channel_id = self.id;
                    if transport.conn.channels.get_mut(&self.id).map(|c| c.confirm).unwrap_or(false) {
                        Box::new(future::poll_fn(move || {
                            if let Ok(mut tr) = cl_transport.try_lock() {
                                tr.handle_frames()?;
                                let acked_opt = tr.conn.channels.get_mut(&channel_id).map(|c| {
                                    if c.acked.remove(&delivery_tag) {
                                        Some(true)
                                    } else if c.nacked.remove(&delivery_tag) {
                                        Some(false)
                                    } else {
                                        info!("message with tag {} still in unacked: {:?}", delivery_tag, c.unacked);
                                        None
                                    }
                                }).unwrap();

                                if acked_opt.is_some() {
                                    return Ok(Async::Ready(acked_opt));
                                } else {
                                    tr.poll()?;
                                    return Ok(Async::NotReady);
                                }
                            } else {
                                return Ok(Async::NotReady);
                            };
                        }))
                    } else {
                        Box::new(future::ok(None))
                    }
                },
            }
        } else {
            //FIXME: if we're there, it means the mutex failed
            Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, "Failed to acquire AMQPTransport mutex")))
        }
    }

    /// creates a consumer stream
    ///
    /// returns a future of a `Consumer` that resolves once the method succeeds
    ///
    /// `Consumer` implements `futures::Stream`, so it can be used with any of
    /// the usual combinators
    pub fn basic_consume(&self, queue: &str, consumer_tag: &str, options: &BasicConsumeOptions, arguments: FieldTable) -> Box<Future<Item = Consumer<T>, Error = io::Error>> {
        let consumer = Consumer {
            transport:    self.transport.clone(),
            channel_id:   self.id,
            queue:        queue.to_string(),
            consumer_tag: consumer_tag.to_string(),
        };

        Box::new(self.run_on_locked_transport("basic_consume", "Could not start consumer", |mut transport| {
            transport.conn.basic_consume(self.id, options.ticket, queue.to_string(), consumer_tag.to_string(),
            options.no_local, options.no_ack, options.exclusive, options.no_wait, arguments.clone()).map(Some)
        }).map(|_| {
            trace!("basic_consume received response, returning consumer");
            consumer
        }))
    }

    /// acks a message
    pub fn basic_ack(&self, delivery_tag: u64) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("basic_ack", "Could not ack message", |mut transport| {
            transport.conn.basic_ack(self.id, delivery_tag, false).map(|_| None)
        })
    }

    /// rejects a message
    pub fn basic_reject(&self, delivery_tag: u64, requeue: bool) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("basic_reject", "Could not reject message", |mut transport| {
            transport.conn.basic_reject(self.id, delivery_tag, requeue).map(|_| None)
        })
    }

    /// gets a message
    pub fn basic_get(&self, queue: &str, options: &BasicGetOptions) -> Box<Future<Item = Message, Error = io::Error>> {
        let channel_id = self.id;
        let _queue = queue.to_string();
        let receive_transport = self.transport.clone();
        let receive_future = future::poll_fn(move || {
            if let Ok(mut transport) = receive_transport.try_lock() {
                transport.handle_frames()?;
                if let Some(message) = transport.conn.next_get_message(channel_id, &_queue) {
                    Ok(Async::Ready(message))
                } else {
                    transport.poll()?;
                    trace!("basic get[{}-{}] not ready", channel_id, _queue);
                    Ok(Async::NotReady)
                }
            } else {
                return Ok(Async::NotReady);
            }
        });

        Box::new(self.run_on_locked_transport_full("basic_get", "Could not get message", |mut transport| {
            transport.conn.basic_get(self.id, options.ticket, queue.to_string(), options.no_ack).map(Some)
        }, Connection::finished_get_result, || Err(Error::new(ErrorKind::Other, "basic get returned empty"))).and_then(|_| receive_future))
    }

    /// Purge a queue.
    ///
    /// This method removes all messages from a queue which are not awaiting acknowledgment.
    pub fn queue_purge(&self, queue_name: &str, options: &QueuePurgeOptions) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("queue_purge", "Could not purge queue", |mut transport| {
            transport.conn.queue_purge(self.id, options.ticket, queue_name.to_string(), options.nowait).map(Some)
        })
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
    pub fn queue_delete(&self, queue_name: &str, options: &QueueDeleteOptions) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("queue_purge", "Could not purge queue", |mut transport| {
            transport.conn.queue_delete(self.id, options.ticket, queue_name.to_string(), options.if_unused, options.if_empty, options.no_wait).map(Some)
        })
    }

    /// closes the cannel
    pub fn close(&self, code: u16, message: &str) -> Box<Future<Item = (), Error = io::Error>> {
        self.run_on_locked_transport("close", "Could not close channel", |mut transport| {
            transport.conn.channel_close(self.id, code, message.to_string(), 0, 0).map(|_| None)
        })
    }

    fn run_on_locked_transport_full<Action, Finished, NoAnswer>(&self, method: &str, error: &str, mut action: Action, finished: Finished, no_answer: NoAnswer) -> Box<Future<Item = (), Error = io::Error>>
        where Action:   FnMut(&mut AMQPTransport<T>) -> Result<Option<RequestId>, lapin_async::error::Error>,
              Finished: 'static + Fn(&mut Connection, RequestId) -> Option<bool>,
              NoAnswer: 'static + Fn() -> Poll<bool, io::Error> {
        if let Ok(mut transport) = self.transport.lock() {
            match action(&mut transport) {
                Err(e)         => Box::new(future::err(Error::new(ErrorKind::Other, format!("{}: {:?}", error, e)))),
                Ok(request_id) => Self::process_frames(&mut transport, method, request_id.map(|request_id| (request_id, self.transport.clone(), finished, no_answer))),
            }
        } else {
            //FIXME: if we're there, it means the mutex failed
            Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, "Failed to acquire AMQPTransport mutex")))
        }
    }

    fn run_on_locked_transport<Action>(&self, method: &str, error: &str, action: Action) -> Box<Future<Item = (), Error = io::Error>>
        where Action: FnMut(&mut AMQPTransport<T>) -> Result<Option<RequestId>, lapin_async::error::Error> {
        self.run_on_locked_transport_full(method, error, action, Connection::is_finished, || Ok(Async::NotReady))
    }

    fn process_frames<Finished, NoAnswer>(transport: &mut AMQPTransport<T>, method: &str, request_id_data: Option<(RequestId, Arc<Mutex<AMQPTransport<T>>>, Finished, NoAnswer)>) -> Box<Future<Item = (), Error = io::Error>>
        where Finished: 'static + Fn(&mut Connection, RequestId) -> Option<bool>,
              NoAnswer: 'static + Fn() -> Poll<bool, io::Error> {
        trace!("{} request id: {:?}", method, request_id_data.as_ref().map(|r| r.0));
        if let Err(e) = transport.send_and_handle_frames() {
            let err = format!("Failed to handle frames: {:?}", e);
            trace!("{}", err);
            return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
        }

        if let Some((request_id, cl_transport, finished, no_answer)) = request_id_data {
            trace!("{} returning closure", method);
            Box::new(Self::wait_for_answer(cl_transport, request_id, finished, no_answer).map(|_| ()))
        } else {
            Box::new(future::ok(()))
        }
    }

    /// internal method to wait until a request succeeds
    pub fn wait_for_answer<Finished, NoAnswer>(transport: Arc<Mutex<AMQPTransport<T>>>, request_id: RequestId, finished: Finished, no_answer: NoAnswer) -> Box<Future<Item = bool, Error = io::Error>>
        where Finished: 'static + Fn(&mut Connection, RequestId) -> Option<bool>,
              NoAnswer: 'static + Fn() -> Poll<bool, io::Error> {
        trace!("wait for answer for request {}", request_id);
        Box::new(future::poll_fn(move || {
            let got_answer = if let Ok(mut tr) = transport.try_lock() {
                tr.handle_frames()?;
                if let Some(res) = finished(&mut tr.conn, request_id) {
                    res
                } else {
                    return Ok(Async::NotReady);
                }
            } else {
                return Ok(Async::NotReady);
            };

            if got_answer {
                Ok(Async::Ready(got_answer))
            } else {
                no_answer()
            }
        }))
    }
}
