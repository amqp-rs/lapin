use crate::{
    BasicProperties, Error, Result,
    channel_closer::ChannelCloser,
    consumer_canceler::ConsumerCanceler,
    consumer_status::{ConsumerState, ConsumerStatus},
    error_holder::ErrorHolder,
    internal_rpc::InternalRPCHandle,
    message::{Delivery, DeliveryResult},
    options::BasicConsumeOptions,
    types::{ChannelId, PayloadSize},
    types::{FieldTable, ShortString},
    wakers::Wakers,
};
use executor_trait::FullExecutor;
use flume::{Receiver, Sender};
use futures_core::stream::Stream;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
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
/// There are two ways of acknowledging a message:
///
/// * If the flag [`BasicConsumeOptions::no_ack`] is set to `true` while obtaining the consumer from
///   [`Channel::basic_consume`], the server implicitely acknowledges each message after it has been
///   sent.
/// * If the flag [`BasicConsumeOptions::no_ack`] is set to `false`, a message has to be explicitly
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
    consumer_tag: ShortString,
    inner: Arc<Mutex<Inner>>,
    status: ConsumerStatus,
    channel_closer: Option<Arc<ChannelCloser>>,
    consumer_canceler: Option<Arc<ConsumerCanceler>>,
    queue: ShortString,
    options: BasicConsumeOptions,
    arguments: FieldTable,
    deliveries_in: Sender<DeliveryResult>,
    wakers: Wakers,
    error: ErrorHolder,
    executor: Arc<dyn FullExecutor + Send + Sync>,
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
        let (sender, receiver) = flume::unbounded();
        let status = ConsumerStatus::default();
        Self {
            consumer_tag: consumer_tag.clone(),
            inner: Arc::new(Mutex::new(Inner::new(consumer_tag, receiver))),
            status,
            channel_closer,
            consumer_canceler: None,
            queue,
            options,
            arguments,
            deliveries_in: sender,
            wakers: Wakers::default(),
            error: ErrorHolder::default(),
            executor,
        }
    }

    pub(crate) fn external(
        &self,
        channel_id: ChannelId,
        internal_rpc_handle: InternalRPCHandle,
    ) -> Self {
        Self {
            consumer_tag: self.consumer_tag.clone(),
            inner: self.inner.clone(),
            status: self.status.clone(),
            channel_closer: None,
            consumer_canceler: Some(Arc::new(ConsumerCanceler::new(
                channel_id,
                self.consumer_tag.to_string(),
                self.status.clone(),
                internal_rpc_handle,
            ))),
            queue: self.queue.clone(),
            options: self.options,
            arguments: self.arguments.clone(),
            deliveries_in: self.deliveries_in.clone(),
            wakers: self.wakers.clone(),
            error: self.error.clone(),
            executor: self.executor.clone(),
        }
    }

    pub(crate) fn error(&self) -> ErrorHolder {
        self.error.clone()
    }

    /// Gets the consumer tag.
    ///
    /// If no consumer tag was specified when obtaining the consumer from the channel,
    /// this contains the server generated consumer tag.
    pub fn tag(&self) -> ShortString {
        self.consumer_tag.clone()
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
        let mut inner = self.lock_inner();
        let mut status = self.status.write();
        while let Some(delivery) = inner.next_delivery() {
            self.executor.spawn(delegate.on_new_delivery(delivery));
        }
        status.set_delegate(Some(Arc::new(Box::new(delegate))));
    }

    pub(crate) fn reset(&self) {
        self.lock_inner()
            .reset(self.options.no_ack, &self.executor, self.status.delegate());
    }

    pub(crate) fn start_new_delivery(&self, delivery: Delivery) {
        self.lock_inner().current_message = Some(delivery);
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        size: PayloadSize,
        properties: BasicProperties,
    ) {
        self.check_new_delivery(
            self.lock_inner()
                .handle_content_header_frame(size, properties),
        );
    }

    pub(crate) fn handle_body_frame(&self, remaining_size: PayloadSize, payload: Vec<u8>) {
        self.check_new_delivery(self.lock_inner().handle_body_frame(remaining_size, payload));
    }

    pub(crate) fn drop_prefetched_messages(&self) {
        self.lock_inner()
            .drop_prefetched_messages(&self.executor, self.status.delegate());
    }

    pub(crate) fn start_cancel(&self) {
        self.status.write().start_cancel();
    }

    pub(crate) fn cancel(&self) {
        trace!(consumer_tag=%self.consumer_tag, "cancel");
        let mut status = self.status.write();
        self.dispatch(
            Ok(None),
            "failed to send cancel to consumer",
            status.delegate(),
        );
        status.cancel();
    }

    pub(crate) fn send_error(&self, error: Error) {
        trace!(consumer_tag=%self.consumer_tag, "send_error");
        self.dispatch(
            Err(error),
            "failed to send error to consumer",
            self.status.delegate(),
        );
    }

    pub(crate) fn set_error(&self, error: Error) {
        trace!(consumer_tag=%self.consumer_tag, "set_error");
        self.error.set(error.clone());
        self.send_error(error);
        self.cancel();
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn check_new_delivery(&self, delivery: Option<Delivery>) {
        if let Some(delivery) = delivery {
            self.dispatch(
                Ok(Some(delivery)),
                "failed to send delivery to consumer",
                self.status.delegate(),
            );
        }
    }

    fn dispatch(
        &self,
        delivery: DeliveryResult,
        error: &'static str,
        delegate: Option<Arc<Box<dyn ConsumerDelegate>>>,
    ) {
        if let Some(delegate) = delegate {
            self.executor.spawn(delegate.on_new_delivery(delivery));
        } else {
            self.deliveries_in.send(delivery).expect(error);
        }
        self.wakers.wake();
    }
}

impl fmt::Debug for Consumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Consumer");
        if let Ok(inner) = self.inner.try_lock() {
            debug.field("tag", &inner.tag);
        }
        if let Some(status) = self.status.try_read() {
            debug.field("state", &status.state());
        }
        debug.finish()
    }
}

// This impl is there only to silence warnings
impl Drop for Consumer {
    fn drop(&mut self) {
        drop(self.consumer_canceler.take());
        drop(self.channel_closer.take());
    }
}

struct Inner {
    current_message: Option<Delivery>,
    deliveries_out: Receiver<DeliveryResult>,
    tag: ShortString,
}

impl Inner {
    fn new(consumer_tag: ShortString, deliveries_out: Receiver<DeliveryResult>) -> Self {
        Self {
            current_message: None,
            deliveries_out,
            tag: consumer_tag,
        }
    }

    fn reset(
        &mut self,
        no_ack: bool,
        executor: &dyn FullExecutor,
        delegate: Option<Arc<Box<dyn ConsumerDelegate>>>,
    ) {
        if !no_ack {
            self.drop_prefetched_messages(executor, delegate);
        }
        self.current_message = None;
    }

    fn next_delivery(&mut self) -> Option<DeliveryResult> {
        self.deliveries_out.try_recv().ok()
    }

    fn handle_content_header_frame(
        &mut self,
        size: PayloadSize,
        properties: BasicProperties,
    ) -> Option<Delivery> {
        if let Some(delivery) = self.current_message.as_mut() {
            delivery.properties = properties;
        }
        self.check_new_delivery_complete(size == 0)
    }

    fn handle_body_frame(
        &mut self,
        remaining_size: PayloadSize,
        payload: Vec<u8>,
    ) -> Option<Delivery> {
        if let Some(delivery) = self.current_message.as_mut() {
            delivery.receive_content(payload);
        }
        self.check_new_delivery_complete(remaining_size == 0)
    }

    fn check_new_delivery_complete(&mut self, complete: bool) -> Option<Delivery> {
        if !complete {
            return None;
        }
        self.current_message
            .take()
            .inspect(|_| trace!(consumer_tag=%self.tag, "new_delivery"))
    }

    fn drop_prefetched_messages(
        &mut self,
        executor: &dyn FullExecutor,
        delegate: Option<Arc<Box<dyn ConsumerDelegate>>>,
    ) {
        trace!(consumer_tag=%self.tag, "drop_prefetched_messages");
        if let Some(delegate) = delegate {
            executor.spawn(delegate.drop_prefetched_messages());
        }
        while let Some(delivery) = self.next_delivery() {
            if let Ok(Some(delivery)) = delivery {
                delivery.acker.invalidate();
            }
        }
    }
}

impl Stream for Consumer {
    type Item = Result<Delivery>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("consumer poll_next");
        self.wakers.register(cx.waker());
        let mut inner = self.lock_inner();
        trace!(
            consumer_tag=%inner.tag,
            "consumer poll; acquired inner lock"
        );
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

    use crate::ErrorKind;

    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };

    use futures_lite::stream::StreamExt;

    struct Counter(AtomicUsize);

    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn stream_on_cancel() {
        let awoken_count = Arc::new(Counter(AtomicUsize::new(0)));
        let waker = Waker::from(awoken_count.clone());
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

            assert_eq!(awoken_count.0.load(Ordering::SeqCst), 0);
            assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Pending);
        }

        consumer.cancel();

        {
            let mut next = consumer.next();

            assert_eq!(awoken_count.0.load(Ordering::SeqCst), 1);
            assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Ready(None));
        }
    }

    #[test]
    fn stream_on_error() {
        let awoken_count = Arc::new(Counter(AtomicUsize::new(0)));
        let waker = Waker::from(awoken_count.clone());
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

            assert_eq!(awoken_count.0.load(Ordering::SeqCst), 0);
            assert_eq!(Pin::new(&mut next).poll(&mut cx), Poll::Pending);
        }

        consumer.set_error(ErrorKind::ChannelsLimitReached.into());

        {
            let mut next = consumer.next();

            assert_eq!(awoken_count.0.load(Ordering::SeqCst), 1);
            assert_eq!(
                Pin::new(&mut next).poll(&mut cx),
                Poll::Ready(Some(Err(ErrorKind::ChannelsLimitReached.into())))
            );
        }
    }
}
