use std::io::{self,Error,ErrorKind};
use futures::{Async,Future,future,Stream};
use amq_protocol::types::*;
use tokio_io::{AsyncRead,AsyncWrite};
use std::sync::{Arc,Mutex};
use std::default::Default;
use lapin_async::api::RequestId;
use lapin_async::queue::Message;
use lapin_async::generated::basic;

use transport::*;
use consumer::Consumer;
use client::wait_for_answer;


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

#[derive(Clone,Debug,PartialEq)]
pub struct ExchangeDeclareOptions {
  pub passive:     bool,
  pub durable:     bool,
  pub auto_delete: bool,
  pub internal:    bool,
  pub nowait:      bool,
}

impl Default for ExchangeDeclareOptions {
  fn default() -> ExchangeDeclareOptions {
    ExchangeDeclareOptions {
      passive:     false,
      durable:     false,
      auto_delete: false,
      internal:    false,
      nowait:      false,
    }
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct QueueDeclareOptions {
  pub passive:     bool,
  pub durable:     bool,
  pub exclusive:   bool,
  pub auto_delete: bool,
  pub nowait:      bool,
}

impl Default for QueueDeclareOptions {
  fn default() -> QueueDeclareOptions {
    QueueDeclareOptions {
      passive:     false,
      durable:     false,
      exclusive:   false,
      auto_delete: false,
      nowait:      false,
    }
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct QueueBindOptions {
  pub nowait: bool,
}

impl Default for QueueBindOptions {
  fn default() -> QueueBindOptions {
    QueueBindOptions {
      nowait: false,
    }
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct BasicPublishOptions {
  pub ticket:    u16,
  pub exchange:  String,
  pub mandatory: bool,
  pub immediate: bool,
}

impl Default for BasicPublishOptions {
  fn default() -> BasicPublishOptions {
    BasicPublishOptions {
      ticket:    0,
      exchange:  "".to_string(),
      mandatory: false,
      immediate: false,
    }
  }
}

pub type BasicProperties = basic::Properties;

#[derive(Clone,Debug,PartialEq)]
pub struct BasicConsumeOptions {
  pub ticket:    u16,
  pub no_local:  bool,
  pub no_ack:    bool,
  pub exclusive: bool,
  pub no_wait:   bool,
}

impl Default for BasicConsumeOptions {
  fn default() -> BasicConsumeOptions {
    BasicConsumeOptions {
      ticket:    0,
      no_local:  false,
      no_ack:    false,
      exclusive: false,
      no_wait:   false,
    }
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct BasicGetOptions {
  pub ticket:    u16,
  pub no_ack:    bool,
}

impl Default for BasicGetOptions {
  fn default() -> BasicGetOptions {
    BasicGetOptions {
      ticket:    0,
      no_ack:    false,
    }
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct QueueDeleteOptions {
  pub if_unused: bool,
  pub if_empty:  bool,
  pub no_wait:   bool,
}

impl Default for QueueDeleteOptions {
  fn default() -> QueueDeleteOptions {
    QueueDeleteOptions {
      if_unused: false,
      if_empty:  false,
      no_wait:   false,
    }
  }
}

impl<T: AsyncRead+AsyncWrite+'static> Channel<T> {
  /// creates an exchange
  ///
  /// returns a future that resolves once the exchange is available
  pub fn exchange_declare(&self, name: &str, exchange_type: &str, options: &ExchangeDeclareOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.exchange_declare(
        self.id, 0, name.to_string(), exchange_type.to_string(),
        options.passive, options.durable, options.auto_delete, options.internal, options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not declare exchange: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("exchange_declare request id: {}", request_id);
          transport.send_and_handle_frames();

          trace!("exchange_declare returning closure");
          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// creates a queue
  ///
  /// returns a future that resolves once the queue is available
  ///
  /// the `mandatory` and `ìmmediate` options can be set to true,
  /// but the return message will not be handled
  pub fn queue_declare(&self, name: &str, options: &QueueDeclareOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_declare(
        self.id, 0, name.to_string(),
        options.passive, options.durable, options.exclusive, options.auto_delete, options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not declare queue: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("queue_declare request id: {}", request_id);
          transport.send_and_handle_frames();

          trace!("queue_declare returning closure");
          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// binds a queue to an exchange
  ///
  /// returns a future that resolves once the queue is bound to the exchange
  pub fn queue_bind(&self, name: &str, exchange: &str, routing_key: &str, options: &QueueBindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_bind(
        self.id, 0, name.to_string(), exchange.to_string(), routing_key.to_string(),
        options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not bind queue: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("queue_bind request id: {}", request_id);
          transport.send_and_handle_frames();

          trace!("queue_bind returning closure");
          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// sets up confirm extension for this channel
  pub fn confirm_select(&self) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.confirm_select(self.id, false) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not activate confirm extension: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("confirm select request id: {}", request_id);
          transport.send_and_handle_frames();

          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
      Error::new(ErrorKind::ConnectionAborted, format!("could not activate confirm extension"))
      ))
    }
  }

  /// publishes a message on a queue
  ///
  /// the future's result is:
  /// - `Some(true)` if we're on a confirm channel and the message was ack'd
  /// - `Some(false)` if we're on a confirm channel and the message was nack'd
  /// - `None` if we're not on a confirm channel
  pub fn basic_publish(&self, queue: &str, payload: &[u8], options: &BasicPublishOptions, properties: BasicProperties) -> Box<Future<Item = Option<bool>, Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      //FIXME: does not handle the return messages with mandatory and immediate
      match transport.conn.basic_publish(self.id, options.ticket, options.exchange.clone(),
        queue.to_string(), options.mandatory, options.immediate) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish: {:?}", e)))
        ),
        Ok(delivery_tag) => {
          transport.send_and_handle_frames();
          transport.conn.send_content_frames(self.id, 60, payload, properties);
          transport.send_and_handle_frames();

          if transport.conn.channels.get_mut(&self.id).map(|c| c.confirm).unwrap_or(false) {
            wait_for_basic_publish_confirm(cl_transport, delivery_tag, self.id)
          } else {
            Box::new(future::ok(None))
          }
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// creates a consumer stream
  ///
  /// returns a future of a `Consumer` that resolves once the method succeeds
  ///
  /// `Consumer` implements `futures::Stream`, so it can be used with any of
  /// the usual combinators
  pub fn basic_consume(&self, queue: &str, consumer_tag: &str, options: &BasicConsumeOptions) -> Box<Future<Item = Consumer<T>, Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.basic_consume(self.id, options.ticket, queue.to_string(), consumer_tag.to_string(),
        options.no_local, options.no_ack, options.exclusive, options.no_wait, FieldTable::new()) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not start consumer")))
        ),
        Ok(request_id) => {
          transport.send_and_handle_frames();

          let consumer = Consumer {
            transport:    cl_transport.clone(),
            channel_id:   self.id,
            queue:        queue.to_string(),
            consumer_tag: consumer_tag.to_string(),
          };

          trace!("basic_consume returning closure");
          Box::new(wait_for_answer(cl_transport, request_id).map(move |_| {
            trace!("basic_consume received response, returning consumer");
            consumer
          }))
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// acks a message
  pub fn basic_ack(&self, delivery_tag: u64) -> Box<Future<Item = (), Error = io::Error>> {
    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.basic_ack(self.id, delivery_tag, false) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish: {:?}", e)))
        ),
        Ok(_) => {
          transport.send_and_handle_frames();
          Box::new(future::ok(()))
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// rejects a message
  pub fn basic_reject(&self, delivery_tag: u64, requeue: bool) -> Box<Future<Item = (), Error = io::Error>> {
    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.basic_reject(self.id, delivery_tag, requeue) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish: {:?}", e)))
        ),
        Ok(_) => {
          transport.send_and_handle_frames();
          Box::new(future::ok(()))
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// acks a message
  pub fn basic_get(&self, queue: &str, options: &BasicGetOptions) -> Box<Future<Item = Message, Error = io::Error>> {
    let cl_transport = self.transport.clone();
    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.basic_get(self.id, options.ticket, queue.to_string(), options.no_ack) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish: {:?}", e)))
        ),
        Ok(request_id) => {
          transport.send_and_handle_frames();
          wait_for_basic_get_answer(cl_transport, request_id, self.id, queue)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not create channel"))
      ))
    }
  }

  /// Purge a queue.
  ///
  /// This method removes all messages from a queue which are not awaiting acknowledgment.
  pub fn queue_purge(&self, queue_name: &str) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_purge(self.id, 0, queue_name.to_string(), false) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not purge queue: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("purge request id: {}", request_id);
          transport.send_and_handle_frames();

          trace!("purge returning closure");
          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not purge queue {}", queue_name))
      ))
    }
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
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_delete(self.id, 0, queue_name.to_string(), options.if_unused, options.if_empty, options.no_wait) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not delete queue: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("delete request id: {}", request_id);
          transport.send_and_handle_frames();

          trace!("delete returning closure");
          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not delete queue {}", queue_name))
      ))
    }
  }
}

/// internal method to wait until a basic get succeeded
pub fn wait_for_basic_get_answer<T: AsyncRead+AsyncWrite+'static>(transport: Arc<Mutex<AMQPTransport<T>>>,
  request_id: RequestId, channel_id: u16, queue: &str) -> Box<Future<Item = Message, Error = io::Error>> {

  let receive_transport = transport.clone();
  let queue = queue.to_string();

  let request_future = future::poll_fn(move || {
    let result = if let Ok(mut tr) = transport.try_lock() {
      tr.handle_frames();
      match tr.conn.finished_get_result(request_id) {
        None =>  {
          tr.handle_frames();
          if let Some(res) = tr.conn.finished_get_result(request_id) {
            res
          } else {
            return Ok(Async::NotReady);
          }
        },
        Some(res) => res,
      }
    } else {
      return Ok(Async::NotReady);
    };

    if result {
      Ok(Async::Ready(()))
    } else {
      Err(Error::new(ErrorKind::Other, format!("basic get returned empty")))
    }
  });

  let receive_future = future::poll_fn(move || {
    if let Ok(mut transport) = receive_transport.try_lock() {
      transport.handle_frames();
      if let Some(message) = transport.conn.next_get_message(channel_id, &queue) {
        Ok(Async::Ready(message))
      } else {
        transport.upstream.poll();
        transport.heartbeat.poll();
        trace!("basic get[{}-{}] not ready", channel_id, queue);
        Ok(Async::NotReady)
      }
    } else {
      return Ok(Async::NotReady);
    }
  });

  Box::new(request_future.and_then(|_| receive_future))
}

/// internal method to wait until a basic publish confirmation comes back
pub fn wait_for_basic_publish_confirm<T: AsyncRead+AsyncWrite+'static>(transport: Arc<Mutex<AMQPTransport<T>>>,
  delivery_tag: u64, channel_id: u16) -> Box<Future<Item = Option<bool>, Error = io::Error>> {

  Box::new(future::poll_fn(move || {
    if let Ok(mut tr) = transport.try_lock() {
      tr.handle_frames();
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
        tr.upstream.poll();
        return Ok(Async::NotReady);
      }
    } else {
      return Ok(Async::NotReady);
    };

  }))
}
