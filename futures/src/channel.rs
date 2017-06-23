use std::io::{self,Error,ErrorKind};
use futures::{Async,Future,future,Stream};
use tokio_io::{AsyncRead,AsyncWrite};
use std::sync::{Arc,Mutex};
use lapin_async::api::RequestId;
use lapin_async::queue::Message;
use lapin_async::generated::basic;

use transport::*;
use types::FieldTable;
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
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.access_request(
        self.id, realm.to_string(), options.exclusive, options.passive, options.active, options.write, options.read) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not request access {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "access_request", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  /// creates an exchange
  ///
  /// returns a future that resolves once the exchange is available
  pub fn exchange_declare(&self, name: &str, exchange_type: &str, options: &ExchangeDeclareOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.exchange_declare(
        self.id, options.ticket, name.to_string(), exchange_type.to_string(),
        options.passive, options.durable, options.auto_delete, options.internal, options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not declare exchange: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "exchange_declare", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  /// deletes an exchange
  ///
  /// returns a future that resolves once the exchange is deleted
  pub fn exchange_delete(&self, name: &str, options: &ExchangeDeleteOptions) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.exchange_delete(
        self.id, options.ticket, name.to_string(), options.if_unused, options.nowait) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not delete exchange: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "exchange_delete", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  /// binds an exchange to another exchange
  ///
  /// returns a future that resolves once the exchanges are bound
  pub fn exchange_bind(&self, destination: &str, source: &str, routing_key: &str, options: &ExchangeBindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.exchange_bind(
        self.id, options.ticket, destination.to_string(), source.to_string(), routing_key.to_string(), options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not bind exchange: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "exchange_bind", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  /// unbinds an exchange from another one
  ///
  /// returns a future that resolves once the exchanges are unbound
  pub fn exchange_unbind(&self, destination: &str, source: &str, routing_key: &str, options: &ExchangeUnbindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.exchange_unbind(
        self.id, options.ticket, destination.to_string(), source.to_string(), routing_key.to_string(), options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not unbind exchange: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "exchange_unbind", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
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
        self.id, options.ticket, name.to_string(),
        options.passive, options.durable, options.exclusive, options.auto_delete, options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not declare queue: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "queue_declare", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  /// binds a queue to an exchange
  ///
  /// returns a future that resolves once the queue is bound to the exchange
  pub fn queue_bind(&self, name: &str, exchange: &str, routing_key: &str, options: &QueueBindOptions, arguments: FieldTable) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_bind(
        self.id, options.ticket, name.to_string(), exchange.to_string(), routing_key.to_string(),
        options.nowait, arguments) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not bind queue: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "queue_bind", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
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
          Self::process_frames(&mut transport, Some(cl_transport), "confirm_select", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
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

          if transport.conn.channels.get_mut(&self.id).map(|c| c.confirm).unwrap_or(false) {
            wait_for_basic_publish_confirm(cl_transport, delivery_tag, self.id)
          } else {
            Box::new(future::ok(None))
          }
        },
      }
    } else {
        Self::mutex_failed()
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
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not start consumer: {:?}", e)))
        ),
        Ok(request_id) => {
          //TODO: Self::process_frames(&mut transport, Some(cl_transport), "basic_consume", Some(request_id))
          if let Err(e) = transport.send_and_handle_frames() {
            let err = format!("Failed to handle frames: {:?}", e);
            trace!("{}", err);
            return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
          }

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
        Self::mutex_failed()
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
          Self::process_frames(&mut transport, None, "basic_ack", None)
        },
      }
    } else {
        Self::mutex_failed()
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
          Self::process_frames(&mut transport, None, "basic_reject", None)
        },
      }
    } else {
        Self::mutex_failed()
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
          //TODO: Self::process_frames(&mut transport, Some(cl_transport), "basic_get", Some(request_id))
          if let Err(e) = transport.send_and_handle_frames() {
            let err = format!("Failed to handle frames: {:?}", e);
            trace!("{}", err);
            return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
          }
          wait_for_basic_get_answer(cl_transport, request_id, self.id, queue)
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  /// Purge a queue.
  ///
  /// This method removes all messages from a queue which are not awaiting acknowledgment.
  pub fn queue_purge(&self, queue_name: &str, options: &QueuePurgeOptions) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_purge(self.id, options.ticket, queue_name.to_string(), options.nowait) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not purge queue: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "queue_purge", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
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
      match transport.conn.queue_delete(self.id, options.ticket, queue_name.to_string(), options.if_unused, options.if_empty, options.no_wait) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not delete queue: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, Some(cl_transport), "queue_delete", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  /// closes the cannel
  pub fn close(&self, code: u16, message: String) -> Box<Future<Item = (), Error = io::Error>> {
    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.channel_close(self.id, code, message, 0, 0) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::Other, format!("could not close channel: {:?}", e)))
        ),
        Ok(request_id) => {
          Self::process_frames(&mut transport, None, "close", Some(request_id))
        },
      }
    } else {
        Self::mutex_failed()
    }
  }

  fn mutex_failed<I: 'static>() -> Box<Future<Item = I, Error = io::Error>> {
      //FIXME: if we're there, it means the mutex failed
      Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, "Failed to acquire AMQPTransport mutex")))
  }

  fn process_frames(transport: &mut AMQPTransport<T>, cl_transport: Option<Arc<Mutex<AMQPTransport<T>>>>, method: &str, request_id: Option<RequestId>) -> Box<Future<Item = (), Error = io::Error>> {
      trace!("{} request id: {:?}", method, request_id);
      if let Err(e) = transport.send_and_handle_frames() {
          let err = format!("Failed to handle frames: {:?}", e);
          trace!("{}", err);
          return Box::new(future::err(Error::new(ErrorKind::ConnectionAborted, err)));
      }

      cl_transport.and_then(|cl_transport| {
          request_id.map(|request_id| {
              trace!("{} returning closure", method);
              wait_for_answer(cl_transport, request_id)
          })
      }).unwrap_or_else(|| Box::new(future::ok(())))
  }
}

/// internal method to wait until a basic get succeeded
pub fn wait_for_basic_get_answer<T: AsyncRead+AsyncWrite+'static>(transport: Arc<Mutex<AMQPTransport<T>>>,
  request_id: RequestId, channel_id: u16, queue: &str) -> Box<Future<Item = Message, Error = io::Error>> {

  let receive_transport = transport.clone();
  let queue = queue.to_string();

  let request_future = future::poll_fn(move || {
    let result = if let Ok(mut tr) = transport.try_lock() {
      tr.handle_frames()?;
      match tr.conn.finished_get_result(request_id) {
        None =>  {
          tr.handle_frames()?;
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
      transport.handle_frames()?;
      if let Some(message) = transport.conn.next_get_message(channel_id, &queue) {
        Ok(Async::Ready(message))
      } else {
        transport.poll()?;
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
}
