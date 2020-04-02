use crate::{
    connection::Connection, executor::Executor, frames::Frames, id_sequence::IdSequence,
    BasicProperties, Channel, ChannelState, Error, Result,
};
use amq_protocol::protocol::AMQPClass;
use log::debug;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Debug)]
pub(crate) struct Channels {
    inner: Arc<Mutex<Inner>>,
    frames: Frames,
}

impl Channels {
    pub(crate) fn new(frames: Frames, executor: Arc<dyn Executor>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::new(executor))),
            frames,
        }
    }

    pub(crate) fn create(&self, connection: Connection) -> Result<Channel> {
        self.inner.lock().create(connection)
    }

    pub(crate) fn create_zero(&self, connection: Connection) {
        self.inner
            .lock()
            .create_channel(0, connection)
            .set_state(ChannelState::Connected);
    }

    pub(crate) fn get(&self, id: u16) -> Option<Channel> {
        self.inner.lock().channels.get(&id).cloned()
    }

    pub(crate) fn remove(&self, id: u16, error: Error) -> Result<()> {
        self.frames.clear_expected_replies(id, error);
        if self.inner.lock().channels.remove(&id).is_some() {
            Ok(())
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn receive_method(&self, id: u16, method: AMQPClass) -> Result<()> {
        if let Some(channel) = self.get(id) {
            channel.receive_method(method)
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        id: u16,
        size: u64,
        properties: BasicProperties,
    ) -> Result<()> {
        if let Some(channel) = self.get(id) {
            channel.handle_content_header_frame(size, properties)
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn handle_body_frame(&self, id: u16, payload: Vec<u8>) -> Result<()> {
        if let Some(channel) = self.get(id) {
            channel.handle_body_frame(payload)
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn set_closing(&self) {
        for channel in self.inner.lock().channels.values() {
            channel.set_state(ChannelState::Closing);
        }
    }

    pub(crate) fn set_closed(&self, error: Error) -> Result<()> {
        self.inner
            .lock()
            .channels
            .drain()
            .map(|(id, channel)| {
                self.frames.clear_expected_replies(id, error.clone());
                channel.set_state(ChannelState::Closed);
                channel.error_publisher_confirms(error.clone());
                channel.cancel_consumers()
            })
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn set_error(&self, error: Error) -> Result<()> {
        self.inner
            .lock()
            .channels
            .drain()
            .map(|(id, channel)| {
                self.frames.clear_expected_replies(id, error.clone());
                channel.set_state(ChannelState::Error);
                channel.error_publisher_confirms(error.clone());
                channel.error_consumers(error.clone())
            })
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn flow(&self) -> bool {
        self.inner
            .lock()
            .channels
            .values()
            .all(|c| c.status().flow())
    }
}

#[derive(Debug)]
struct Inner {
    channels: HashMap<u16, Channel>,
    channel_id: IdSequence<u16>,
    executor: Arc<dyn Executor>,
}

impl Inner {
    fn new(executor: Arc<dyn Executor>) -> Self {
        Self {
            channels: HashMap::default(),
            channel_id: IdSequence::new(false),
            executor,
        }
    }
}

impl Inner {
    fn create_channel(&mut self, id: u16, connection: Connection) -> Channel {
        debug!("create channel with id {}", id);
        let channel = Channel::new(id, connection, self.executor.clone());
        self.channels.insert(id, channel.clone());
        channel
    }

    fn create(&mut self, connection: Connection) -> Result<Channel> {
        debug!("create channel");
        self.channel_id
            .set_max(connection.configuration().channel_max());
        let first_id = self.channel_id.next();
        let mut looped = false;
        let mut id = first_id;
        while !looped || id < first_id {
            if id == 1 {
                looped = true;
            }
            if !self.channels.contains_key(&id) {
                return Ok(self.create_channel(id, connection));
            }
            id = self.channel_id.next();
        }
        Err(Error::ChannelsLimitReached)
    }
}
