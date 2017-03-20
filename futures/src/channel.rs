use std::io::{self,Error,ErrorKind};
use futures::Future;
use futures::future;
use std::collections::HashMap;
use tokio_io::{AsyncRead,AsyncWrite};
use std::sync::{Arc,Mutex};

use transport::*;
use consumer::Consumer;
use client::wait_for_answer;

#[derive(Clone)]
pub struct Channel<T> {
  pub transport: Arc<Mutex<AMQPTransport<T>>>,
  pub id:    u16,
}

impl<T: AsyncRead+AsyncWrite+'static> Channel<T> {
  pub fn queue_declare(&self, name: &str) -> Box<Future<Item = (), Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.queue_declare(self.id, 0, name.to_string(), false, false, false, false, false, HashMap::new()) {
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

  pub fn basic_publish(&self, queue: &str, payload: &[u8]) -> Box<Future<Item = (), Error = io::Error>> {
    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.basic_publish(self.id, 0, "".to_string(), queue.to_string(), false, false) {
        Err(e) => Box::new(
          future::err(Error::new(ErrorKind::ConnectionAborted, format!("could not publish: {:?}", e)))
        ),
        Ok(_) => {
          transport.send_frames();
          transport.conn.send_content_frames(self.id, 60, payload);
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

  pub fn basic_consume(&self, queue: &str, consumer_tag: &str) -> Box<Future<Item = Consumer<T>, Error = io::Error>> {
    let cl_transport = self.transport.clone();

    if let Ok(mut transport) = self.transport.lock() {
      match transport.conn.basic_consume(self.id, 0, queue.to_string(), consumer_tag.to_string(), false, true, false, false, HashMap::new()) {
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
