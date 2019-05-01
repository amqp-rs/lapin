use amq_protocol::{
  frame::AMQPFrame,
  protocol::AMQPClass,
};
use crossbeam_channel::Sender;
use parking_lot::Mutex;

use std::{
  collections::HashMap,
  sync::Arc,
};

use crate::{
  channel::{BasicProperties, Channel},
  configuration::Configuration,
  error::{Error, ErrorKind},
  id_sequence::IdSequence,
};

#[derive(Clone, Debug)]
pub struct Channels {
  inner:         Arc<Mutex<Inner>>,
}

impl Channels {
  pub fn new(configuration: Configuration, frame_sender: Sender<AMQPFrame>) -> Self {
    let channels = Self {
      inner: Arc::new(Mutex::new(Inner::new(configuration, frame_sender))),
    };
    channels.inner.lock().create_channel(0, channels.clone());
    channels
  }

  pub fn create(&self) -> Result<Channel, Error> {
    self.inner.lock().create(self.clone())
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
  channels:      HashMap<u16, Channel>,
  channel_id:    IdSequence<u16>,
  configuration: Configuration,
  frame_sender:  Sender<AMQPFrame>,
}

impl Inner {
  fn new(configuration: Configuration, frame_sender: Sender<AMQPFrame>) -> Self {
    Self {
      channels:   Default::default(),
      channel_id: IdSequence::new(false),
      configuration,
      frame_sender,
    }
  }

  fn create_channel(&mut self, id: u16, channels: Channels) -> Channel {
    let channel = Channel::new(id, self.configuration.clone(), self.frame_sender.clone(), channels);
    self.channels.insert(id, channel.clone());
    channel
  }

  fn create(&mut self, channels: Channels) -> Result<Channel, Error> {
    let channel_max = self.configuration.channel_max();
    let first_id = self.channel_id.next_with_max(channel_max);
    let mut looped = false;
    let mut id = first_id;
    while !looped || id < first_id {
      if id == 1 {
        looped = true;
      }
      if !self.channels.contains_key(&id) {
        return Ok(self.create_channel(id, channels))
      }
      id = self.channel_id.next_with_max(channel_max);
    }
    Err(ErrorKind::ChannelLimitReached.into())
  }
}
