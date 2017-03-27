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

impl<T: AsyncRead+AsyncWrite+'static> Channel<T> {
  /// creates a queue
  ///
  /// returns a future that resolves once the queue is available
  ///
  /// the `mandatory` and `ìmmediate` options can be set to true,
  /// but the return message will not be handled
  pub fn queue_declare(&self, name: &str, options: &QueueDeclareOptions) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_declare(
        self.id, 0, name.to_string(),
        options.passive, options.durable, options.exclusive, options.auto_delete, options.nowait, FieldTable::new()) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not declare queue: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("queue_declare request id: {}", request_id);
          transport.send_frames();

          transport.handle_frames();

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

  /// publishes a message on a queue
  pub fn basic_publish(&self, queue: &str, payload: &[u8], options: &BasicPublishOptions, properties: BasicProperties) -> Box<Future<Item = (), Error = io::Error>> {
    if let Ok(mut transport) = self.transport.lock() {
      //FIXME: does not handle the return messages with mandatory and immediate
      match transport.conn.basic_publish(self.id, options.ticket, options.exchange.clone(),
        queue.to_string(), options.mandatory, options.immediate) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish: {:?}", e)))
        ),
        Ok(_) => {
          transport.send_frames();
          transport.conn.send_content_frames(self.id, 60, payload, properties);
          transport.send_frames();

          transport.handle_frames();

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
          transport.send_frames();

          transport.handle_frames();

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
          transport.send_frames();
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
          transport.send_frames();
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
          transport.send_frames();
          transport.handle_frames();
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

  /// purges a queue
  pub fn queue_purge(&self, name: &str) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_purge(self.id, 0, name.to_string(), false) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not purge queue: {:?}", e)))
        ),
        Ok(request_id) => {
          trace!("purge request id: {}", request_id);
          transport.send_frames();

          transport.handle_frames();

          trace!("purge returning closure");
          wait_for_answer(cl_transport, request_id)
        },
      }
    } else {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(
        Error::new(ErrorKind::ConnectionAborted, format!("could not purge queue {}", name))
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
