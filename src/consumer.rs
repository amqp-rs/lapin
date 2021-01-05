use crate::{
    channel_closer::ChannelCloser,
    consumer_canceler::ConsumerCanceler,
    consumer_status::{ConsumerState, ConsumerStatus},
    internal_rpc::InternalRPCHandle,
    message::{Delivery, DeliveryResult},
    options::BasicConsumeOptions,
    types::{ChannelId, PayloadSize},
    types::{FieldTable, ShortString},
    wakers::Wakers,
    BasicProperties, Error, Result,
};
use executor_trait::FullExecutor;
use flume::{Receiver, Sender};
use futures_lite::Stream;
use parking_lot::Mutex;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tracing::trace;

pub trait ConsumerDelegate: Send + Sync {
    fn on_new_delivery(&self, delivery: DeliveryResult)
        -> Pin<Box<dyn Future<Output = ()> + Send>>;
    fn drop_prefetched_messages(&self) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {})
    }
}

impl<
        F: Future<Output = ()> + Send + 'static,
        DeliveryHandler: Fn(DeliveryResult) -> F + Send + Sync + 'static,
    > ConsumerDelegate for DeliveryHandler
{
    fn on_new_delivery(
        &self,
        delivery: DeliveryResult,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(self(delivery))
    }
}

/// Continuously consumes message from a Queue.
///
/// A consumer represents a stream of messages created from
/// the [basic.consume](https://www.rabbitmq.com/amqp-0-9-1-quickref.html#basic.consume) AMQP command.
/// It continuously receives messages from the queue, as opposed to the
/// [basic.get](https://www.rabbitmq.com/amqp-0-9-1-quickref.html#basic.get) command, which
/// retrieves only a single message.
///
/// A consumer is obtained by calling [`Channel::basic_consume`] with the queue name.
///
/// New messages from this consumer can be accessed by obtaining the iterator from the consumer.
/// This iterator returns new messages and the associated channel in the form of a
/// [`DeliveryResult`] for as long as the consumer is subscribed to the queue.
///
/// It is also possible to set a delegate to be spawned via [`set_delegate`].
///
/// ## Message acknowledgment
///
/// There are two ways for acknowledging a message:
///
/// * If the flag [`BasicConsumeOptions::no_ack`] is set to `true` while obtaining the consumer from
///   [`Channel::basic_consume`], the server implicitely acknowledges each message after it has been
///   sent.
/// * If the flag [`BasicConsumeOptions::no_ack`] is set to `false`, a message has to be explicitely
///   acknowledged or rejected with [`Acker::ack`],
///   [`Acker::nack`] or [`Acker::reject`]. See the documentation at [`Delivery`]
///   for further information.
///
/// Also see the RabbitMQ documentation about
/// [Acknowledgement Modes](https://www.rabbitmq.com/consumers.html#acknowledgement-modes).
///
/// ## Consumer Prefetch
///
/// To limit the maximum number of unacknowledged messages arriving, you can call [`Channel::basic_qos`]
/// before creating the consumer.
///
/// Also see the RabbitMQ documentation about
/// [Consumer Prefetch](https://www.rabbitmq.com/consumer-prefetch.html).
///
/// ## Cancel subscription
///
/// To stop receiving messages, call [`Channel::basic_cancel`] with the consumer tag of this
/// consumer.
///
///
/// ## Example
/// ```rust,no_run
/// use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties, Result};
/// use futures_lite::stream::StreamExt;
/// use std::future::Future;
///
/// let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
///
/// let res: Result<()> = async_global_executor::block_on(async {
///     let conn = Connection::connect(
///         &addr,
///         ConnectionProperties::default(),
///     )
///     .await?;
///     let channel = conn.create_channel().await?;
///     let mut consumer = channel
///         .basic_consume(
///             "hello",
///             "my_consumer",
///             BasicConsumeOptions::default(),
///             FieldTable::default(),
///         )
///         .await?;
///
///     while let Some(delivery) = consumer.next().await {
///         let delivery = delivery.expect("error in consumer");
///         delivery
///             .ack(BasicAckOptions::default())
///             .await?;
///     }
///     Ok(())
/// });
/// ```
///
/// [`Channel::basic_consume`]: ./struct.Channel.html#method.basic_consume
/// [`Channel::basic_qos`]: ./struct.Channel.html#method.basic_qos
/// [`Channel::basic_cancel`]: ./struct.Channel.html#method.basic_cancel
/// [`Acker::ack`]: ./struct.Acker.html#method.ack
/// [`Acker::reject`]: ./struct.Acker.html#method.reject
/// [`Acker::nack`]: ./struct.Acker.html#method.nack
/// [`DeliveryResult`]: ./message/type.DeliveryResult.html
/// [`BasicConsumeOptions::no_ack`]: ./options/struct.BasicConsumeOptions.html#structfield.no_ack
/// [`set_delegate`]: #method.set_delegate
#[derive(Clone)]
pub struct Consumer {
    inner: Arc<Mutex<ConsumerInner>>,
    status: ConsumerStatus,
    channel_closer: Option<Arc<ChannelCloser>>,
    consumer_canceler: Option<Arc<ConsumerCanceler>>,
    queue: ShortString,
    options: BasicConsumeOptions,
    arguments: FieldTable,
}

impl Consumer {
    pub(crate) fn new(
        consumer_tag: ShortString,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        channel_closer: Option<Arc<ChannelCloser>>,
        queue: ShortString,
        options: BasicConsumeOptions,
        arguments: FieldTable,
    ) -> Self {
        let status = ConsumerStatus::default();
        Self {
            inner: Arc::new(Mutex::new(ConsumerInner::new(
                status.clone(),
                consumer_tag,
                executor,
            ))),
            status,
            channel_closer,
            consumer_canceler: None,
            queue,
            options,
            arguments,
        }
    }

    pub(crate) fn external(
        &self,
        channel_id: ChannelId,
        internal_rpc_handle: InternalRPCHandle,
    ) -> Self {
        Self {
            inner: self.inner.clone(),
            status: self.status.clone(),
            channel_closer: None,
            consumer_canceler: Some(Arc::new(ConsumerCanceler::new(
                channel_id,
                self.tag().to_string(),
                self.status.clone(),
                internal_rpc_handle,
            ))),
            queue: self.queue.clone(),
            options: self.options,
            arguments: self.arguments.clone(),
        }
    }

    /// Gets the consumer tag.
    ///
    /// If no consumer tag was specified when obtaining the consumer from the channel,
    /// this contains the server generated consumer tag.
    pub fn tag(&self) -> ShortString {
        self.inner.lock().tag.clone()
    }

    /// Gets the current state of the Consumer.
    pub fn state(&self) -> ConsumerState {
        self.status.state()
    }

    /// Get the name of the queue we're consuming
    pub fn queue(&self) -> ShortString {
        self.queue.clone()
    }

    pub(crate) fn options(&self) -> BasicConsumeOptions {
        self.options
    }

    pub(crate) fn arguments(&self) -> FieldTable {
        self.arguments.clone()
    }

    /// Automatically spawns the delegate on the executor for each message.
    ///
    /// Enables parallel handling of the messages.
    pub fn set_delegate<D: ConsumerDelegate + 'static>(&self, delegate: D) {
        let mut inner = self.inner.lock();
        let mut status = self.status.lock();
        while let Some(delivery) = inner.next_delivery() {
            inner.executor.spawn(delegate.on_new_delivery(delivery));
        }
        inner.delegate = Some(Arc::new(Box::new(delegate)));
        status.set_delegate();
    }

    pub(crate) fn reset(&self) {
        self.inner.lock().reset(self.options.no_ack);
    }

    pub(crate) fn start_new_delivery(&self, delivery: Delivery) {
        self.inner.lock().current_message = Some(delivery);
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        size: PayloadSize,
        properties: BasicProperties,
    ) {
        self.inner
            .lock()
            .handle_content_header_frame(size, properties);
    }

    pub(crate) fn handle_body_frame(&self, remaining_size: PayloadSize, payload: Vec<u8>) {
        self.inner.lock().handle_body_frame(remaining_size, payload);
    }

    pub(crate) fn drop_prefetched_messages(&self) {
        self.inner.lock().drop_prefetched_messages();
    }

    pub(crate) fn start_cancel(&self) {
        self.status.lock().start_cancel();
    }

    pub(crate) fn cancel(&self) {
        self.inner.lock().cancel();
    }

    pub(crate) fn set_error(&self, error: Error) {
        self.inner.lock().set_error(error);
    }
}

struct ConsumerInner {
    status: ConsumerStatus,
    current_message: Option<Delivery>,
    deliveries_in: Sender<DeliveryResult>,
    deliveries_out: Receiver<DeliveryResult>,
    wakers: Wakers,
    tag: ShortString,
    delegate: Option<Arc<Box<dyn ConsumerDelegate>>>,
    executor: Arc<dyn FullExecutor + Send + Sync>,
}

impl fmt::Debug for Consumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Consumer");
        if let Some(inner) = self.inner.try_lock() {
            debug.field("tag", &inner.tag);
        }
        if let Some(status) = self.status.try_lock() {
            debug.field("state", &status.state());
        }
        debug.finish()
    }
}

impl ConsumerInner {
    fn new(
        status: ConsumerStatus,
        consumer_tag: ShortString,
        executor: Arc<dyn FullExecutor + Send + Sync>,
    ) -> Self {
        let (sender, receiver) = flume::unbounded();
        Self {
            status,
            current_message: None,
            deliveries_in: sender,
            deliveries_out: receiver,
            wakers: Wakers::default(),
            tag: consumer_tag,
            delegate: None,
            executor,
        }
    }

    fn reset(&mut self, no_ack: bool) {
        if !no_ack {
            while self.next_delivery().is_some() {}
        }
        self.current_message = None;
    }

    fn next_delivery(&mut self) -> Option<DeliveryResult> {
        self.deliveries_out.try_recv().ok()
    }

    fn handle_content_header_frame(&mut self, size: PayloadSize, properties: BasicProperties) {
        if let Some(delivery) = self.current_message.as_mut() {
            delivery.properties = properties;
        }
        if size == 0 {
            self.new_delivery_complete();
        }
    }

    fn handle_body_frame(&mut self, remaining_size: PayloadSize, payload: Vec<u8>) {
        if let Some(delivery) = self.current_message.as_mut() {
            delivery.receive_content(payload);
        }
        if remaining_size == 0 {
            self.new_delivery_complete();
        }
    }

    fn new_delivery_complete(&mut self) {
        if let Some(delivery) = self.current_message.take() {
            trace!(consumer_tag=%self.tag, "new_delivery");
            if let Some(delegate) = self.delegate.as_ref() {
                let delegate = delegate.clone();
                self.executor
                    .spawn(delegate.on_new_delivery(Ok(Some(delivery))));
            } else {
                self.deliveries_in
                    .send(Ok(Some(delivery)))
                    .expect("failed to send delivery to consumer");
            }
            self.wakers.wake();
        }
    }

    fn drop_prefetched_messages(&mut self) {
        trace!(consumer_tag=%self.tag, "drop_prefetched_messages");
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor.spawn(delegate.drop_prefetched_messages());
        }
        while self.next_delivery().is_some() {}
    }

    fn cancel(&mut self) {
        trace!(consumer_tag=%self.tag, "cancel");
        let mut status = self.status.lock();
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor.spawn(delegate.on_new_delivery(Ok(None)));
        } else {
            self.deliveries_in
                .send(Ok(None))
                .expect("failed to send cancel to consumer");
        }
        self.wakers.wake();
        status.cancel();
    }

    fn set_error(&mut self, error: Error) {
        trace!(consumer_tag=%self.tag, "set_error");
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor.spawn(delegate.on_new_delivery(Err(error)));
        } else {
            self.deliveries_in
                .send(Err(error))
                .expect("failed to send error to consumer");
        }
        self.cancel();
    }
}

impl Stream for Consumer {
    type Item = Result<Delivery>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("consumer poll_next");
        let mut inner = self.inner.lock();
        trace!(
            consumer_tag=%inner.tag,
            "consumer poll; acquired inner lock"
        );
        inner.wakers.register(cx.waker());
        if let Some(delivery) = inner.next_delivery() {
            match delivery {
                Ok(Some(delivery)) => {
                    trace!(
                        consumer_tag=%inner.tag,
                        delivery_tag=?delivery.delivery_tag,
                        "delivery"
                    );
                    Poll::Ready(Some(Ok(delivery)))
                }
                Ok(None) => {
                    trace!(consumer_tag=%inner.tag, "consumer canceled");
                    Poll::Ready(None)
                }
                Err(error) => Poll::Ready(Some(Err(error))),
            }
        } else {
            trace!(consumer_tag=%inner.tag, "delivery; status=NotReady");
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod futures_tests {
    use super::*;

    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::task::{Context, Poll};

    use futures_lite::stream::StreamExt;
    use waker_fn::waker_fn;

    #[test]
    fn stream_on_cancel() {
        let awoken_count = Arc::new(AtomicUsize::new(0));
        let waker = {
            let awoken_count = awoken_count.clone();
            waker_fn(move || {
                awoken_count.fetch_add(1, Ordering::SeqCst);
            })
        };
        let mut cx = Context::from_waker(&waker);

        let mut consumer = Consumer::new(
            ShortString::from("test-consumer"),
            Arc::new(async_global_executor_trait::AsyncGlobalExecutor),
            None,
            "test".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        );
        {
            let mut next = consumer.next();

            assert_eq!(awoken_count.load(Ordering::SeqCst), 0);
            assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Pending);
        }

        consumer.cancel();

        {
            let mut next = consumer.next();

            assert_eq!(awoken_count.load(Ordering::SeqCst), 1);
            assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Ready(None));
        }
    }

    #[test]
    fn stream_on_error() {
        let awoken_count = Arc::new(AtomicUsize::new(0));
        let waker = {
            let awoken_count = awoken_count.clone();
            waker_fn(move || {
                awoken_count.fetch_add(1, Ordering::SeqCst);
            })
        };
        let mut cx = Context::from_waker(&waker);

        let mut consumer = Consumer::new(
            ShortString::from("test-consumer"),
            Arc::new(async_global_executor_trait::AsyncGlobalExecutor),
            None,
            "test".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        );
        {
            let mut next = consumer.next();

            assert_eq!(awoken_count.load(Ordering::SeqCst), 0);
            assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Pending);
        }

        consumer.set_error(Error::ChannelsLimitReached);

        {
            let mut next = consumer.next();

            assert_eq!(awoken_count.load(Ordering::SeqCst), 1);
            assert_eq!(
                Pin::new(&mut next).poll(&mut cx),
                Poll::Ready(Some(Err(Error::ChannelsLimitReached)))
            );
        }
    }
}
