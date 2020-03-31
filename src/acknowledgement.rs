use crate::{
    pinky_swear::PinkyBroadcaster,
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

    pub(crate) fn register_pending(&self, delivery_tag: DeliveryTag) -> PublisherConfirm {
        self.inner.lock().register_pending(delivery_tag)
    }

    pub(crate) fn get_last_pending(&self) -> Option<PublisherConfirm> {
        self.inner.lock().last.take()
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
}

#[derive(Debug)]
struct Inner {
    last: Option<PublisherConfirm>,
    pending: HashMap<DeliveryTag, PinkyBroadcaster<Confirmation>>,
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

    fn register_pending(&mut self, delivery_tag: DeliveryTag) -> PublisherConfirm {
        let broadcaster = PinkyBroadcaster::default();
        let promise = broadcaster.subscribe().into();
        self.last = Some(broadcaster.subscribe().into());
        self.pending.insert(delivery_tag, broadcaster);
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
        for pinky in self
            .pending
            .drain()
            .map(|tup| tup.1)
            .collect::<Vec<PinkyBroadcaster<Confirmation>>>()
        {
            self.complete_pending(success, pinky);
        }
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> Result<()> {
        if let Some(pinky) = self.pending.remove(&delivery_tag) {
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
            .iter()
            .map(|tup| tup.0)
            .filter(|tag| **tag <= delivery_tag)
            .cloned()
            .collect()
    }
}
