use crate::{
    publisher_confirm::{Confirmation, PublisherConfirm},
    returned_messages::ReturnedMessages,
    ConfirmationBroadcaster, Error, Promise, Result,
};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::Arc,
};

pub type DeliveryTag = u64;

#[derive(Clone)]
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
        channel_id: u16,
    ) -> PublisherConfirm {
        self.inner.lock().register_pending(delivery_tag, channel_id)
    }

    pub(crate) fn get_last_pending(&self) -> Option<Promise<Confirmation>> {
        self.inner.lock().last.take().map(|(_, promise)| promise)
    }

    pub(crate) fn ack(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner.lock().drop_pending(delivery_tag, true)
    }

    pub(crate) fn nack(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner.lock().drop_pending(delivery_tag, false)
    }

    pub(crate) fn ack_all_pending(&self) {
        self.inner.lock().drop_all(true);
    }

    pub(crate) fn nack_all_pending(&self) {
        self.inner.lock().drop_all(false);
    }

    pub(crate) fn ack_all_before(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner
            .lock()
            .complete_pending_before(delivery_tag, true)
    }

    pub(crate) fn nack_all_before(&self, delivery_tag: DeliveryTag) -> Result<()> {
        self.inner
            .lock()
            .complete_pending_before(delivery_tag, false)
    }

    pub(crate) fn on_channel_error(&self, channel_id: u16, error: Error) {
        self.inner.lock().on_channel_error(channel_id, error);
    }
}

impl fmt::Debug for Acknowledgements {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.inner.lock();
        f.debug_struct("Acknowledgements")
            .field("returned_messages", &inner.returned_messages)
            .field("pending", &inner.pending.keys())
            .finish()
    }
}

struct Inner {
    last: Option<(DeliveryTag, Promise<Confirmation>)>,
    pending: HashMap<DeliveryTag, (u16, ConfirmationBroadcaster)>,
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
        let broadcaster = ConfirmationBroadcaster::default();
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

    fn complete_pending(&mut self, success: bool, resolver: ConfirmationBroadcaster) {
        let returned_message = self.returned_messages.get_waiting_message().map(Box::new);
        resolver.swear(Ok(if success {
            Confirmation::Ack(returned_message)
        } else {
            Confirmation::Nack(returned_message)
        }));
    }

    fn drop_all(&mut self, success: bool) {
        for (_, resolver) in self
            .pending
            .drain()
            .map(|tup| tup.1)
            .collect::<Vec<(u16, ConfirmationBroadcaster)>>()
        {
            self.complete_pending(success, resolver);
        }
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> Result<()> {
        if let Some((_, resolver)) = self.pending.remove(&delivery_tag) {
            self.complete_pending(success, resolver);
            Ok(())
        } else {
            Err(Error::InvalidAck)
        }
    }

    fn complete_pending_before(&mut self, delivery_tag: DeliveryTag, success: bool) -> Result<()> {
        let mut res = Ok(());
        for tag in self
            .pending
            .keys()
            .filter(|tag| **tag <= delivery_tag)
            .cloned()
            .collect::<HashSet<DeliveryTag>>()
        {
            if let Err(err) = self.drop_pending(tag, success) {
                res = Err(err);
            }
        }
        res
    }

    fn on_channel_error(&mut self, channel_id: u16, error: Error) {
        for (_, resolver) in self
            .pending
            .values()
            .filter(|(channel, _)| *channel == channel_id)
        {
            resolver.swear(Err(error.clone()));
        }
        self.pending
            .retain(|_, (channel, _)| *channel != channel_id);
    }
}
