use amq_protocol::{
  frame::AMQPFrame,
  protocol::AMQPClass,
};
use log::debug;
use parking_lot::Mutex;

use std::{
  collections::HashMap,
  sync::Arc,
};

use crate::{
  channel::{BasicProperties, Channel},
  connection::Connection,
  error::{Error, ErrorKind},
  id_sequence::IdSequence,
};

#[derive(Clone, Debug, Default)]
pub struct Channels {
  inner:         Arc<Mutex<Inner>>,
}

impl Channels {
  pub fn create(&self, connection: Connection) -> Result<Channel, Error> {
    self.inner.lock().create(connection)
  }

  pub(crate) fn create_zero(&self, connection: Connection) -> () {
    self.inner.lock().create_channel(0, connection);
  }

  pub fn get(&self, id: u16) -> Option<Channel> {
    self.inner.lock().channels.get(&id).cloned()
  }

  pub fn remove(&self, id: u16) -> Result<(), Error> {
    if self.inner.lock().channels.remove(&id).is_some() {
      Ok(())
    } else {
      Err(ErrorKind::InvalidChannel(id).into())
    }
  }

  pub fn receive_method(&self, id: u16, method: AMQPClass) -> Result<(), Error> {
    if let Some(channel) = self.get(id) {
      channel.receive_method(method)
    } else {
      Err(ErrorKind::InvalidChannel(id).into())
    }
  }

  pub fn send_frame(&self, id: u16, frame: AMQPFrame) -> Result<(), Error> {
    if let Some(channel) = self.get(id) {
      channel.send_frame(frame);
      Ok(())
    } else {
      Err(ErrorKind::InvalidChannel(id).into())
    }
  }

  pub fn send_method_frame(&self, id: u16, method: AMQPClass) -> Result<(), Error> {
    if let Some(channel) = self.get(id) {
      channel.send_method_frame(method);
      Ok(())
    } else {
      Err(ErrorKind::InvalidChannel(id).into())
    }
  }

  pub fn handle_content_header_frame(&self, id: u16, size: u64, properties: BasicProperties) -> Result<(), Error> {
    if let Some(channel) = self.get(id) {
      channel.handle_content_header_frame(size, properties)
    } else {
      Err(ErrorKind::InvalidChannel(id).into())
    }
  }

  pub fn handle_body_frame(&self, id: u16, payload: Vec<u8>) -> Result<(), Error> {
    if let Some(channel) = self.get(id) {
      channel.handle_body_frame(payload)
    } else {
      Err(ErrorKind::InvalidChannel(id).into())
    }
  }
}

#[derive(Debug)]
struct Inner {
  channels:   HashMap<u16, Channel>,
  channel_id: IdSequence<u16>,
}

impl Default for Inner {
  fn default() -> Self {
    Self {
      channels:   HashMap::default(),
      channel_id: IdSequence::new(false),
    }
  }
}

impl Inner {
  fn create_channel(&mut self, id: u16, connection: Connection) -> Channel {
    debug!("create channel with id {}", id);
    let channel = Channel::new(id, connection);
    self.channels.insert(id, channel.clone());
    channel
  }

  fn create(&mut self, connection: Connection) -> Result<Channel, Error> {
    debug!("create channel");
    let channel_max = connection.configuration.channel_max();
    let first_id = self.channel_id.next_with_max(channel_max);
    let mut looped = false;
    let mut id = first_id;
    while !looped || id < first_id {
      if id == 1 {
        looped = true;
      }
      if !self.channels.contains_key(&id) {
        return Ok(self.create_channel(id, connection))
      }
      id = self.channel_id.next_with_max(channel_max);
    }
    Err(ErrorKind::ChannelLimitReached.into())
  }
}
