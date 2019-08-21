use parking_lot::Mutex;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::{
    returned_messages::ReturnedMessages,
    wait::{Wait, WaitHandle},
    Error,
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

    pub(crate) fn register_pending(&self, delivery_tag: DeliveryTag) {
        self.inner.lock().register_pending(delivery_tag);
    }

    pub(crate) fn get_last_pending(&self) -> Option<Wait<()>> {
        self.inner.lock().last.take()
    }

    pub(crate) fn ack(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
        self.inner.lock().ack(delivery_tag)
    }

    pub(crate) fn nack(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
        self.inner.lock().nack(delivery_tag)
    }

    pub(crate) fn ack_all_pending(&self) {
        let mut inner = self.inner.lock();
        for wait in inner.drain_pending() {
            wait.finish(());
        }
    }

    pub(crate) fn nack_all_pending(&self) {
        let mut inner = self.inner.lock();
        for wait in inner.drain_pending() {
            wait.finish(());
        }
    }

    pub(crate) fn ack_all_before(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
        let mut inner = self.inner.lock();
        for tag in inner.list_pending_before(delivery_tag) {
            inner.ack(tag)?;
        }
        Ok(())
    }

    pub(crate) fn nack_all_before(&self, delivery_tag: DeliveryTag) -> Result<(), Error> {
        let mut inner = self.inner.lock();
        for tag in inner.list_pending_before(delivery_tag) {
            inner.nack(tag)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Inner {
    last: Option<Wait<()>>,
    pending: HashMap<DeliveryTag, WaitHandle<()>>,
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

    fn register_pending(&mut self, delivery_tag: DeliveryTag) {
        let (wait, wait_handle) = Wait::new();
        self.pending.insert(delivery_tag, wait_handle);
        self.last = Some(wait);
    }

    fn drop_pending(&mut self, delivery_tag: DeliveryTag, success: bool) -> Result<(), Error> {
        if let Some(delivery_wait) = self.pending.remove(&delivery_tag) {
            if success {
                delivery_wait.finish(());
            } else {
                self.returned_messages.register_waiter(delivery_wait);
            }
            Ok(())
        } else {
            Err(Error::PreconditionFailed)
        }
    }

    fn ack(&mut self, delivery_tag: DeliveryTag) -> Result<(), Error> {
        self.drop_pending(delivery_tag, true)?;
        Ok(())
    }

    fn nack(&mut self, delivery_tag: DeliveryTag) -> Result<(), Error> {
        self.drop_pending(delivery_tag, false)?;
        Ok(())
    }

    fn drain_pending(&mut self) -> Vec<WaitHandle<()>> {
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
