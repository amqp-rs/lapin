use std::io;
use futures::{Async,Poll,Stream,task};
use tokio_io::{AsyncRead,AsyncWrite};
use std::sync::{Arc,Mutex};

use message::Delivery;
use transport::*;

#[derive(Clone)]
pub struct Consumer<T> {
  pub transport:    Arc<Mutex<AMQPTransport<T>>>,
  pub channel_id:   u16,
  pub queue:        String,
  pub consumer_tag: String,
}

impl<T: AsyncRead+AsyncWrite+Sync+Send+'static> Stream for Consumer<T> {
  type Item = Delivery;
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Option<Delivery>, io::Error> {
    trace!("consumer[{}] poll", self.consumer_tag);
    let mut transport = try_lock_transport!(self.transport);
    transport.send_and_handle_frames()?;
    //FIXME: if the consumer closed, we should return Ok(Async::Ready(None))
    if let Some(message) = transport.conn.next_delivery(self.channel_id, &self.queue, &self.consumer_tag) {
      trace!("consumer[{}] ready", self.consumer_tag);
      Ok(Async::Ready(Some(message)))
    } else {
      trace!("consumer[{}] not ready", self.consumer_tag);
      Ok(Async::NotReady)
    }
  }
}

