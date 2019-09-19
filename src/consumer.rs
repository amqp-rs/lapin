use crate::{
    executor::Executor, message::Delivery, types::ShortString, wait::NotifyReady, BasicProperties,
    Error,
};
use crossbeam_channel::{Receiver, Sender};
use log::trace;
use parking_lot::{Mutex, MutexGuard};
use std::{fmt, sync::Arc};

pub trait ConsumerDelegate: Send + Sync {
    fn on_new_delivery(&self, delivery: Delivery);
    fn drop_prefetched_messages(&self) {}
    fn on_canceled(&self) {}
    fn on_error(&self, _error: Error) {}
}

#[derive(Clone)]
pub struct Consumer {
    inner: Arc<Mutex<ConsumerInner>>,
}

impl Consumer {
    pub(crate) fn new(consumer_tag: ShortString, executor: Arc<dyn Executor>) -> Consumer {
        Consumer {
            inner: Arc::new(Mutex::new(ConsumerInner::new(consumer_tag, executor))),
        }
    }

    pub fn inner(&self) -> MutexGuard<'_, ConsumerInner> {
        self.inner.lock()
    }

    pub fn set_delegate(&self, delegate: Box<dyn ConsumerDelegate>) {
        let mut inner = self.inner();
        while let Some(delivery) = inner.next_delivery() {
            delegate.on_new_delivery(delivery);
        }
        inner.delegate = Some(Arc::new(Mutex::new(delegate)));
    }

    pub(crate) fn start_new_delivery(&mut self, delivery: Delivery) {
        self.inner().current_message = Some(delivery)
    }

    pub(crate) fn set_delivery_properties(&mut self, properties: BasicProperties) {
        if let Some(delivery) = self.inner().current_message.as_mut() {
            delivery.properties = properties;
        }
    }

    pub(crate) fn receive_delivery_content(&mut self, payload: Vec<u8>) {
        if let Some(delivery) = self.inner().current_message.as_mut() {
            delivery.receive_content(payload);
        }
    }

    pub(crate) fn new_delivery_complete(&mut self) -> Result<(), Error> {
        let mut inner = self.inner();
        if let Some(delivery) = inner.current_message.take() {
            inner.new_delivery(delivery)?;
        }
        Ok(())
    }

    pub(crate) fn drop_prefetched_messages(&self) -> Result<(), Error> {
        self.inner().drop_prefetched_messages()
    }

    pub(crate) fn cancel(&self) -> Result<(), Error> {
        self.inner().cancel()
    }

    pub(crate) fn set_error(&self, error: Error) -> Result<(), Error> {
        self.inner().set_error(error)
    }
}

pub struct ConsumerInner {
    current_message: Option<Delivery>,
    deliveries_in: Sender<Delivery>,
    deliveries_out: Receiver<Delivery>,
    task: Option<Box<dyn NotifyReady + Send>>,
    canceled: bool,
    tag: ShortString,
    delegate: Option<Arc<Mutex<Box<dyn ConsumerDelegate>>>>,
    executor: Arc<dyn Executor>,
    error: Option<Error>,
}

impl fmt::Debug for ConsumerInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConsumerInnder({})", self.tag)
    }
}

impl ConsumerInner {
    fn new(consumer_tag: ShortString, executor: Arc<dyn Executor>) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self {
            current_message: None,
            deliveries_in: sender,
            deliveries_out: receiver,
            task: None,
            canceled: false,
            tag: consumer_tag,
            delegate: None,
            executor,
            error: None,
        }
    }

    pub fn next_delivery(&mut self) -> Option<Delivery> {
        self.deliveries_out.try_recv().ok()
    }

    pub fn set_task(&mut self, task: Box<dyn NotifyReady + Send>) {
        self.task = Some(task);
    }

    pub fn has_task(&self) -> bool {
        self.task.is_some()
    }

    pub fn canceled(&self) -> bool {
        self.canceled
    }

    pub fn error(&mut self) -> Option<Error> {
        self.error.take()
    }

    pub fn tag(&self) -> &ShortString {
        &self.tag
    }

    fn new_delivery(&mut self, delivery: Delivery) -> Result<(), Error> {
        trace!("new_delivery; consumer_tag={}", self.tag);
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor
                .execute(Box::new(move || delegate.lock().on_new_delivery(delivery)))?;
        } else {
            self.deliveries_in
                .send(delivery)
                .expect("failed to send delivery to consumer");
        }
        if let Some(task) = self.task.as_ref() {
            task.notify();
        }
        Ok(())
    }

    fn drop_prefetched_messages(&mut self) -> Result<(), Error> {
        trace!("drop_prefetched_messages; consumer_tag={}", self.tag);
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor
                .execute(Box::new(move || delegate.lock().drop_prefetched_messages()))?;
        }
        while let Some(_) = self.next_delivery() {}
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), Error> {
        trace!("cancel; consumer_tag={}", self.tag);
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor
                .execute(Box::new(move || delegate.lock().on_canceled()))?;
        }
        while let Some(_) = self.next_delivery() {}
        self.canceled = true;
        self.task.take();
        Ok(())
    }

    pub fn set_error(&mut self, error: Error) -> Result<(), Error> {
        trace!("set_error; consumer_tag={}", self.tag);
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor
                .execute(Box::new(move || delegate.lock().on_error(error)))?;
        } else {
            self.error = Some(error);
        }
        self.cancel()
    }
}

impl fmt::Debug for Consumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Consumer({})", self.inner().tag())
    }
}

#[cfg(feature = "futures")]
mod futures {
    use super::*;

    use ::futures::stream::Stream;

    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    use crate::confirmation::futures::Watcher;

    impl Stream for Consumer {
        type Item = Result<Delivery, Error>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            trace!("consumer poll; polling transport");
            let mut inner = self.inner();
            trace!(
                "consumer poll; acquired inner lock, consumer_tag={}",
                inner.tag()
            );
            if !inner.has_task() {
                inner.set_task(Box::new(Watcher(cx.waker().clone())));
            }
            if let Some(delivery) = inner.next_delivery() {
                trace!(
                    "delivery; consumer_tag={}, delivery_tag={:?}",
                    inner.tag(),
                    delivery.delivery_tag
                );
                Poll::Ready(Some(Ok(delivery)))
            } else if inner.canceled() {
                trace!("consumer canceled; consumer_tag={}", inner.tag());
                if let Some(error) = inner.error() {
                    Poll::Ready(Some(Err(error)))
                } else {
                    Poll::Ready(None)
                }
            } else {
                trace!("delivery; status=NotReady, consumer_tag={}", inner.tag());
                Poll::Pending
            }
        }
    }
}
