use crate::{
    pinky_swear::{Pinky, PinkyBroadcaster, PinkySwear},
    returned_messages::ReturnedMessages,
    Error, PublisherConfirm, Result,
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

    pub(crate) fn register_pending(
        &self,
        delivery_tag: DeliveryTag,
    ) -> PinkySwear<Result<PublisherConfirm>> {
        self.inner.lock().register_pending(delivery_tag)
    }

    pub(crate) fn get_last_pending(&self) -> Option<PinkySwear<Result<PublisherConfirm>>> {
        self.inner.lock().last.take()
    }

    pub(crate) fn ack(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner.lock().ack(delivery_tag)
    }

    pub(crate) fn nack(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner.lock().nack(delivery_tag)
    }

    pub(crate) fn ack_all_pending(&self) {
        let mut inner = self.inner.lock();
        for (pinky, _broadcaster) in inner.drain_pending() {
            pinky.swear(Ok(PublisherConfirm::Ack));
        }
    }

    pub(crate) fn nack_all_pending(&self) {
        let mut inner = self.inner.lock();
        for (pinky, _broadcaster) in inner.drain_pending() {
            pinky.swear(Ok(PublisherConfirm::Nack));
        }
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
    last: Option<PinkySwear<Result<PublisherConfirm>>>,
    pending: HashMap<
        DeliveryTag,
        (
            Pinky<Result<PublisherConfirm>>,
            PinkyBroadcaster<Result<PublisherConfirm>>,
        ),
    >,
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

    fn register_pending(
        &mut self,
        delivery_tag: DeliveryTag,
    ) -> PinkySwear<Result<PublisherConfirm>> {
        let (promise, pinky) = PinkySwear::new();
        let broadcaster = PinkyBroadcaster::new(promise);
        let promise = broadcaster.subscribe();
        self.last = Some(broadcaster.subscribe());
        self.pending.insert(delivery_tag, (pinky, broadcaster));
        promise
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> Result<()> {
        if let Some((pinky, broadcaster)) = self.pending.remove(&delivery_tag) {
            if success {
                pinky.swear(Ok(PublisherConfirm::Ack));
            } else {
                self.returned_messages.register_pinky((pinky, broadcaster));
            }
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

    fn drain_pending(
        &mut self,
    ) -> Vec<(
        Pinky<Result<PublisherConfirm>>,
        PinkyBroadcaster<Result<PublisherConfirm>>,
    )> {
        self.pending.drain().map(|tup| tup.1).collect()
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
