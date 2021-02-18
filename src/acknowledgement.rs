use crate::{
    id_sequence::IdSequence,
    protocol::{AMQPError, AMQPSoftError},
    publisher_confirm::{Confirmation, PublisherConfirm},
    returned_messages::ReturnedMessages,
    ConfirmationBroadcaster, DeliveryTag, Error, Promise,
};
use log::trace;
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::Arc,
};

#[derive(Clone)]
pub(crate) struct Acknowledgements(Arc<Mutex<Inner>>);

impl Acknowledgements {
    pub(crate) fn new(channel_id: u16, returned_messages: ReturnedMessages) -> Self {
        Self(Arc::new(Mutex::new(Inner::new(
            channel_id,
            returned_messages,
        ))))
    }

    pub(crate) fn register_pending(&self) -> PublisherConfirm {
        self.0.lock().register_pending()
    }

    pub(crate) fn get_last_pending(&self) -> Option<Promise<Confirmation>> {
        self.0.lock().last.take().map(|(_, promise)| promise)
    }

    pub(crate) fn ack(&self, delivery_tag: DeliveryTag) -> Result<(), AMQPError> {
        self.0.lock().drop_pending(delivery_tag, true)
    }

    pub(crate) fn nack(&self, delivery_tag: DeliveryTag) -> Result<(), AMQPError> {
        self.0.lock().drop_pending(delivery_tag, false)
    }

    pub(crate) fn ack_all_pending(&self) {
        self.0.lock().drop_all(true);
    }

    pub(crate) fn nack_all_pending(&self) {
        self.0.lock().drop_all(false);
    }

    pub(crate) fn ack_all_before(&self, delivery_tag: DeliveryTag) -> Result<(), AMQPError> {
        self.0.lock().complete_pending_before(delivery_tag, true)
    }

    pub(crate) fn nack_all_before(&self, delivery_tag: DeliveryTag) -> Result<(), AMQPError> {
        self.0.lock().complete_pending_before(delivery_tag, false)
    }

    pub(crate) fn on_channel_error(&self, error: Error) {
        self.0.lock().on_channel_error(error);
    }
}

impl fmt::Debug for Acknowledgements {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Acknowledgements");
        if let Some(inner) = self.0.try_lock() {
            debug
                .field("delivery_tag", &inner.delivery_tag)
                .field("returned_messages", &inner.returned_messages)
                .field("pending", &inner.pending.keys());
        }
        debug.finish()
    }
}

struct Inner {
    channel_id: u16,
    delivery_tag: IdSequence<DeliveryTag>,
    last: Option<(DeliveryTag, Promise<Confirmation>)>,
    pending: HashMap<DeliveryTag, ConfirmationBroadcaster>,
    returned_messages: ReturnedMessages,
}

impl Inner {
    fn new(channel_id: u16, returned_messages: ReturnedMessages) -> Self {
        Self {
            channel_id,
            delivery_tag: IdSequence::new(false),
            last: None,
            pending: HashMap::default(),
            returned_messages,
        }
    }

    fn register_pending(&mut self) -> PublisherConfirm {
        let delivery_tag = self.delivery_tag.next();
        trace!("Publishing with delivery_tag {}", delivery_tag);
        let broadcaster = ConfirmationBroadcaster::default();
        let promise =
            PublisherConfirm::new(broadcaster.subscribe(), self.returned_messages.clone());
        if let Some((delivery_tag, promise)) = self.last.take() {
            if let Some(broadcaster) = self.pending.get(&delivery_tag) {
                broadcaster.unsubscribe(promise);
            }
        }
        self.last = Some((delivery_tag, broadcaster.subscribe()));
        self.pending.insert(delivery_tag, broadcaster);
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
        for resolver in self
            .pending
            .drain()
            .map(|(_, resolver)| resolver)
            .collect::<Vec<_>>()
        {
            self.complete_pending(success, resolver);
        }
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> Result<(), AMQPError> {
        if let Some(resolver) = self.pending.remove(&delivery_tag) {
            self.complete_pending(success, resolver);
            Ok(())
        } else {
            Err(AMQPError::new(
                AMQPSoftError::PRECONDITIONFAILED.into(),
                format!(
                    "invalid {} received for inexistant delivery_tag {} on channel {}, was expecting one of {:?}",
                    if success { "ack" } else { "nack" },
                    delivery_tag,
                    self.channel_id,
                    self.pending.keys()
                )
                .into(),
            ))
        }
    }

    fn complete_pending_before(
        &mut self,
        delivery_tag: DeliveryTag,
        success: bool,
    ) -> Result<(), AMQPError> {
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

    fn on_channel_error(&mut self, error: Error) {
        for (_, resolver) in self.pending.drain() {
            resolver.swear(Err(error.clone()));
        }
    }
}
