use crate::{
    Error, Promise, PromiseResolver,
    id_sequence::IdSequence,
    protocol::{AMQPError, AMQPSoftError},
    publisher_confirm::{Confirmation, PublisherConfirm},
    returned_messages::ReturnedMessages,
    shared::SharedMutex,
    types::DeliveryTag,
};
use std::{collections::BTreeMap, fmt, sync::MutexGuard};
use tracing::trace;

#[derive(Clone)]
pub(crate) struct Acknowledgements(SharedMutex<Inner>);

type AMQPResult = Result<(), AMQPError>;

impl Acknowledgements {
    pub(crate) fn new(channel_id: u16, returned_messages: ReturnedMessages) -> Self {
        Self(SharedMutex::new(Inner::new(channel_id, returned_messages)))
    }

    pub(crate) fn register_pending(&self) -> PublisherConfirm {
        self.lock_inner().register_pending()
    }

    pub(crate) fn wait_for_pending(&self) -> Option<Promise<()>> {
        self.lock_inner().wait_for_pending()
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
        self.0.lock()
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
    pending: BTreeMap<DeliveryTag, PromiseResolver<Confirmation>>,
    waiters: Vec<ConfirmWaiter>,
    // A failure that rejected confirms no wait_for_confirms call covered yet.
    unreported_error: Option<Error>,
    returned_messages: ReturnedMessages,
}

struct ConfirmWaiter {
    through: DeliveryTag,
    resolver: PromiseResolver<()>,
}

impl Inner {
    fn new(channel_id: u16, returned_messages: ReturnedMessages) -> Self {
        Self {
            channel_id,
            delivery_tag: IdSequence::new(false),
            pending: BTreeMap::default(),
            waiters: Vec::new(),
            unreported_error: None,
            returned_messages,
        }
    }

    fn register_pending(&mut self) -> PublisherConfirm {
        let delivery_tag = self.delivery_tag.next();
        trace!("Publishing with delivery_tag {}", delivery_tag);
        let (promise, resolver) = Promise::new("acknowledgement");
        let promise = PublisherConfirm::new(promise, self.returned_messages.clone());
        self.pending.insert(delivery_tag, resolver);
        promise
    }

    fn wait_for_pending(&mut self) -> Option<Promise<()>> {
        if let Some(error) = self.unreported_error.take() {
            return Some(Promise::new_with_data(
                "acknowledgement-barrier",
                Err(error),
            ));
        }
        let through = *self.pending.last_key_value()?.0;
        let (promise, resolver) = Promise::new("acknowledgement-barrier");
        self.waiters.push(ConfirmWaiter { through, resolver });
        Some(promise)
    }

    fn complete_pending(&mut self, success: bool, resolver: PromiseResolver<Confirmation>) {
        let returned_message = self.returned_messages.get_waiting_message();
        resolver.resolve(if success {
            Confirmation::Ack(returned_message)
        } else {
            Confirmation::Nack(returned_message)
        });
    }

    fn finish_waiters(&mut self) {
        self.waiters.retain(|waiter| {
            if self.pending.range(..=waiter.through).next().is_some() {
                return true;
            }
            waiter.resolver.resolve(());
            false
        });
    }

    fn drop_all(&mut self, success: bool) {
        for (_, resolver) in std::mem::take(&mut self.pending) {
            self.complete_pending(success, resolver);
        }
        self.finish_waiters();
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> AMQPResult {
        if let Some(resolver) = self.pending.remove(&delivery_tag) {
            self.complete_pending(success, resolver);
            self.finish_waiters();
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
        let tags = self
            .pending
            .keys()
            .filter(|tag| **tag <= delivery_tag)
            .cloned()
            .collect::<Vec<DeliveryTag>>();
        for tag in tags {
            if let Some(resolver) = self.pending.remove(&tag) {
                self.complete_pending(success, resolver);
            }
        }
        self.finish_waiters();
        Ok(())
    }

    fn on_channel_error(&mut self, error: Error) {
        // Waiters report the failure of the confirms they cover; the next
        // wait_for_confirms call reports the failure of later ones.
        let covered = self.waiters.iter().map(|waiter| waiter.through).max();
        if let Some((&last, _)) = self.pending.last_key_value()
            && covered.is_none_or(|covered| covered < last)
        {
            self.unreported_error = Some(error.clone());
        }
        for (_, resolver) in std::mem::take(&mut self.pending) {
            resolver.reject(error.clone());
        }
        for waiter in self.waiters.drain(..) {
            waiter.resolver.reject(error.clone());
        }
        self.returned_messages.clear_dropped_confirms();
    }

    fn reset(&mut self, error: Error) {
        self.on_channel_error(error);
        self.delivery_tag = IdSequence::new(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waiters_cover_all_prior_tags_and_do_not_consume_each_other() {
        let acknowledgements = Acknowledgements::new(1, ReturnedMessages::default());
        let _first = acknowledgements.register_pending();
        let _second = acknowledgements.register_pending();
        let first_waiter = acknowledgements.wait_for_pending().unwrap();
        let second_waiter = acknowledgements.wait_for_pending().unwrap();

        acknowledgements.ack(2).unwrap();
        assert!(first_waiter.try_wait().is_none());
        assert!(second_waiter.try_wait().is_none());

        acknowledgements.ack(1).unwrap();
        assert_eq!(first_waiter.try_wait(), Some(Ok(())));
        assert_eq!(second_waiter.try_wait(), Some(Ok(())));
    }

    #[test]
    fn waiter_snapshot_excludes_later_publishes() {
        let acknowledgements = Acknowledgements::new(1, ReturnedMessages::default());
        let _first = acknowledgements.register_pending();
        let waiter = acknowledgements.wait_for_pending().unwrap();
        let _second = acknowledgements.register_pending();

        acknowledgements.ack(1).unwrap();
        assert_eq!(waiter.try_wait(), Some(Ok(())));
    }

    #[test]
    fn recovery_rejects_all_affected_waiters() {
        let acknowledgements = Acknowledgements::new(1, ReturnedMessages::default());
        let _first = acknowledgements.register_pending();
        let first_waiter = acknowledgements.wait_for_pending().unwrap();
        acknowledgements.nack(1).unwrap();
        assert_eq!(first_waiter.try_wait(), Some(Ok(())));

        let _second = acknowledgements.register_pending();
        let second_waiter = acknowledgements.wait_for_pending().unwrap();
        let error = Error::other("disconnected");
        acknowledgements.reset(error.clone());
        assert_eq!(second_waiter.try_wait(), Some(Err(error)));
    }

    #[test]
    fn failure_of_dropped_confirms_is_reported_by_the_next_wait() {
        for recover in [false, true] {
            let acknowledgements = Acknowledgements::new(1, ReturnedMessages::default());
            drop(acknowledgements.register_pending());
            let error = Error::other("channel closed by the broker");
            if recover {
                acknowledgements.reset(error.clone());
            } else {
                acknowledgements.on_channel_error(error.clone());
            }
            let waiter = acknowledgements.wait_for_pending().unwrap();
            assert_eq!(waiter.try_wait(), Some(Err(error)));
            // Reported once.
            assert!(acknowledgements.wait_for_pending().is_none());
        }
    }

    #[test]
    fn failure_is_reported_again_only_for_confirms_no_waiter_covered() {
        let acknowledgements = Acknowledgements::new(1, ReturnedMessages::default());
        let _first = acknowledgements.register_pending();
        let waiter = acknowledgements.wait_for_pending().unwrap();
        let error = Error::other("channel closed by the broker");
        acknowledgements.on_channel_error(error.clone());
        assert_eq!(waiter.try_wait(), Some(Err(error.clone())));
        assert!(acknowledgements.wait_for_pending().is_none());

        let _second = acknowledgements.register_pending();
        let waiter = acknowledgements.wait_for_pending().unwrap();
        let _third = acknowledgements.register_pending();
        acknowledgements.on_channel_error(error.clone());
        assert_eq!(waiter.try_wait(), Some(Err(error.clone())));
        let waiter = acknowledgements.wait_for_pending().unwrap();
        assert_eq!(waiter.try_wait(), Some(Err(error)));
    }
}
