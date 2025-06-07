use crate::{
    Error, Promise, PromiseResolver,
    id_sequence::IdSequence,
    protocol::{AMQPError, AMQPSoftError},
    publisher_confirm::{Confirmation, PublisherConfirm},
    returned_messages::ReturnedMessages,
    types::DeliveryTag,
};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};
use tracing::trace;

#[derive(Clone)]
pub(crate) struct Acknowledgements(Arc<Mutex<Inner>>);

type AMQPResult = std::result::Result<(), AMQPError>;

impl Acknowledgements {
    pub(crate) fn new(channel_id: u16, returned_messages: ReturnedMessages) -> Self {
        Self(Arc::new(Mutex::new(Inner::new(
            channel_id,
            returned_messages,
        ))))
    }

    pub(crate) fn register_pending(&self) -> PublisherConfirm {
        self.lock_inner().register_pending()
    }

    pub(crate) fn get_last_pending(&self) -> Option<Promise<()>> {
        self.lock_inner().last.take()
    }

    pub(crate) fn ack(&self, delivery_tag: DeliveryTag) -> AMQPResult {
        self.lock_inner().drop_pending(delivery_tag, true)
    }

    pub(crate) fn nack(&self, delivery_tag: DeliveryTag) -> AMQPResult {
        self.lock_inner().drop_pending(delivery_tag, false)
    }

    pub(crate) fn ack_all_pending(&self) {
        self.lock_inner().drop_all(true);
    }

    pub(crate) fn nack_all_pending(&self) {
        self.lock_inner().drop_all(false);
    }

    pub(crate) fn ack_all_before(&self, delivery_tag: DeliveryTag) -> AMQPResult {
        self.lock_inner()
            .complete_pending_before(delivery_tag, true)
    }

    pub(crate) fn nack_all_before(&self, delivery_tag: DeliveryTag) -> AMQPResult {
        self.lock_inner()
            .complete_pending_before(delivery_tag, false)
    }

    pub(crate) fn on_channel_error(&self, error: Error) {
        self.lock_inner().on_channel_error(error);
    }

    pub(crate) fn reset(&self, error: Error) {
        self.lock_inner().reset(error);
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for Acknowledgements {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Acknowledgements");
        if let Ok(inner) = self.0.try_lock() {
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
    last: Option<Promise<()>>,
    pending: HashMap<DeliveryTag, (PromiseResolver<Confirmation>, PromiseResolver<()>)>,
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
        let (promise, resolver) = Promise::new();
        let (err_promise, err_resolver) = Promise::new();
        let promise = PublisherConfirm::new(promise, self.returned_messages.clone());
        self.last = Some(err_promise);
        self.pending.insert(delivery_tag, (resolver, err_resolver));
        promise
    }

    fn complete_pending(
        &mut self,
        success: bool,
        delivery_tag: DeliveryTag,
        resolvers: (PromiseResolver<Confirmation>, PromiseResolver<()>),
    ) {
        let returned_message = self.returned_messages.get_waiting_message().map(Box::new);
        resolvers.0.resolve(if success {
            Confirmation::Ack(returned_message)
        } else {
            Confirmation::Nack(returned_message)
        });
        if Some(delivery_tag) == self.delivery_tag.current() {
            resolvers.1.resolve(());
        }
    }

    fn drop_all(&mut self, success: bool) {
        for (delivery_tag, resolvers) in std::mem::take(&mut self.pending) {
            self.complete_pending(success, delivery_tag, resolvers);
        }
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> AMQPResult {
        if let Some(resolvers) = self.pending.remove(&delivery_tag) {
            self.complete_pending(success, delivery_tag, resolvers);
            Ok(())
        } else {
            Err(AMQPError::new(
                AMQPSoftError::PRECONDITIONFAILED.into(),
                format!(
                    "invalid {} received for inexistant delivery_tag {} on channel {}, current is {:?}, was expecting one of {:?}",
                    if success { "ack" } else { "nack" },
                    delivery_tag,
                    self.channel_id,
                    self.delivery_tag.current(),
                    self.pending.keys()
                )
                .into(),
            ))
        }
    }

    fn complete_pending_before(&mut self, delivery_tag: DeliveryTag, success: bool) -> AMQPResult {
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
        for (delivery_tag, resolvers) in self.pending.drain() {
            resolvers.0.reject(error.clone());
            if Some(delivery_tag) == self.delivery_tag.current() {
                resolvers.1.reject(error.clone());
            }
        }
    }

    fn reset(&mut self, error: Error) {
        self.delivery_tag = IdSequence::new(false);
        self.on_channel_error(error);
    }
}
