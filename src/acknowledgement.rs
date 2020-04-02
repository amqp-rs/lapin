use crate::{
    pinky_swear::{PinkyBroadcaster, PinkySwear},
    publisher_confirm::{Confirmation, PublisherConfirm},
    returned_messages::ReturnedMessages,
    Error, Result,
};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub type DeliveryTag = u64;

#[derive(Debug, Clone)]
pub(crate) struct Acknowledgements {
    inner: Arc<Mutex<Inner>>,
}

impl Acknowledgements {
    pub(crate) fn new(returned_messages: ReturnedMessages) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::new(returned_messages))),
        }
    }

    pub(crate) fn register_pending(&self, delivery_tag: DeliveryTag, channel_id: u16) -> PublisherConfirm {
        self.inner.lock().register_pending(delivery_tag, channel_id)
    }

    pub(crate) fn get_last_pending(&self) -> Option<PinkySwear<Confirmation>> {
        self.inner.lock().last.take().map(|(_, promise)| promise)
    }

    pub(crate) fn ack(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner.lock().ack(delivery_tag)
    }

    pub(crate) fn nack(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner.lock().nack(delivery_tag)
    }

    pub(crate) fn ack_all_pending(&self) {
        self.inner.lock().drop_all(true);
    }

    pub(crate) fn nack_all_pending(&self) {
        self.inner.lock().drop_all(false);
    }

    pub(crate) fn ack_all_before(&self, delivery_tag: DeliveryTag) -> Result<()> {
        let mut inner = self.inner.lock();
        for tag in inner.list_pending_before(delivery_tag) {
            inner.ack(tag)?;
        }
        Ok(())
    }

    pub(crate) fn nack_all_before(&self, delivery_tag: DeliveryTag) -> Result<()> {
        let mut inner = self.inner.lock();
        for tag in inner.list_pending_before(delivery_tag) {
            inner.nack(tag)?;
        }
        Ok(())
    }

    pub(crate) fn on_channel_error(&self, channel_id: u16, error: Error) {
        self.inner.lock().on_channel_error(channel_id, error);
    }
}

#[derive(Debug)]
struct Inner {
    last: Option<(DeliveryTag, PinkySwear<Confirmation>)>,
    pending: HashMap<DeliveryTag, (u16, PinkyBroadcaster<Confirmation>)>,
    returned_messages: ReturnedMessages,
}

impl Inner {
    fn new(returned_messages: ReturnedMessages) -> Self {
        Self {
            last: None,
            pending: HashMap::default(),
            returned_messages,
        }
    }

    fn register_pending(&mut self, delivery_tag: DeliveryTag, channel_id: u16) -> PublisherConfirm {
        let broadcaster = PinkyBroadcaster::default();
        let promise =
            PublisherConfirm::new(broadcaster.subscribe(), self.returned_messages.clone());
        if let Some((delivery_tag, promise)) = self.last.take() {
            if let Some((_, broadcaster)) = self.pending.get(&delivery_tag) {
                broadcaster.unsubscribe(promise);
            }
        }
        self.last = Some((delivery_tag, broadcaster.subscribe()));
        self.pending.insert(delivery_tag, (channel_id, broadcaster));
        promise
    }

    fn complete_pending(&mut self, success: bool, pinky: PinkyBroadcaster<Confirmation>) {
        if success {
            pinky.swear(Confirmation::Ack);
        } else {
            self.returned_messages.register_pinky(pinky);
        }
    }

    fn drop_all(&mut self, success: bool) {
        for (_, pinky) in self
            .pending
            .drain()
            .map(|tup| tup.1)
            .collect::<Vec<(u16, PinkyBroadcaster<Confirmation>)>>()
        {
            self.complete_pending(success, pinky);
        }
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> Result<()> {
        if let Some((_, pinky)) = self.pending.remove(&delivery_tag) {
            self.complete_pending(success, pinky);
            Ok(())
        } else {
            Err(Error::InvalidAck)
        }
    }

    fn ack(&mut self, delivery_tag: DeliveryTag) -> Result<()> {
        self.drop_pending(delivery_tag, true)
    }

    fn nack(&mut self, delivery_tag: DeliveryTag) -> Result<()> {
        self.drop_pending(delivery_tag, false)
    }

    fn list_pending_before(&mut self, delivery_tag: DeliveryTag) -> HashSet<DeliveryTag> {
        self.pending
            .keys()
            .filter(|tag| **tag <= delivery_tag)
            .cloned()
            .collect()
    }

    fn on_channel_error(&mut self, channel_id: u16, error: Error) {
       for (_, pinky) in self.pending.values().filter(|(channel, _)| *channel == channel_id) {
           pinky.swear(Confirmation::Error(Box::new(error.clone())));
       }
       self.pending.retain(|_, (channel, _)| *channel != channel_id);
    }
}
