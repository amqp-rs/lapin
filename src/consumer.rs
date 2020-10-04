use crate::{
    channel_closer::ChannelCloser,
    consumer_canceler::ConsumerCanceler,
    consumer_status::ConsumerState,
    executor::Executor,
    internal_rpc::InternalRPCHandle,
    message::{Delivery, DeliveryResult},
    types::ShortString,
    BasicProperties, Channel, Error, Result,
};
use crossbeam_channel::{Receiver, Sender};
use futures_core::stream::Stream;
use log::trace;
use parking_lot::Mutex;
use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

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
///   acknowledged or rejected with [`Channel::basic_ack`],
///   [`Channel::basic_reject`] or [`Channel::basic_nack`]. See the documentation at [`Delivery`]
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
/// use futures_executor::LocalPool;
/// use futures_util::stream::StreamExt;
/// use std::future::Future;
///
/// let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
///
/// let res: Result<()> = LocalPool::new().run_until(async {
///     let conn = Connection::connect(
///         &addr,
///         ConnectionProperties::default().with_default_executor(8),
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
///         let (channel, delivery) = delivery.expect("error in consumer");
///         channel
///             .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
///             .await?;
///     }
///     Ok(())
/// });
/// ```
///
/// [`Channel::basic_consume`]: ./struct.Channel.html#method.basic_consume
/// [`Channel::basic_qos`]: ./struct.Channel.html#method.basic_qos
/// [`Channel::basic_ack`]: ./struct.Channel.html#method.basic_ack
/// [`Channel::basic_reject`]: ./struct.Channel.html#method.basic_reject
/// [`Channel::basic_nack`]: ./struct.Channel.html#method.basic_nack
/// [`Channel::basic_cancel`]: ./struct.Channel.html#method.basic_cancel
/// [`DeliveryResult`]: ./message/type.DeliveryResult.html
/// [`BasicConsumeOptions::no_ack`]: ./options/struct.BasicConsumeOptions.html#structfield.no_ack
/// [`set_delegate`]: #method.set_delegate
#[derive(Clone)]
pub struct Consumer {
    inner: Arc<Mutex<ConsumerInner>>,
    state: Arc<Mutex<ConsumerState>>,
    channel_closer: Option<Arc<ChannelCloser>>,
    consumer_canceler: Option<Arc<ConsumerCanceler>>,
}

impl Consumer {
    pub(crate) fn new(
        consumer_tag: ShortString,
        executor: Arc<dyn Executor>,
        channel_closer: Option<Arc<ChannelCloser>>,
    ) -> Self {
        let state = Arc::new(Mutex::new(ConsumerState::default()));
        Self {
            inner: Arc::new(Mutex::new(ConsumerInner::new(
                state.clone(),
                consumer_tag,
                executor,
            ))),
            state,
            channel_closer,
            consumer_canceler: None,
        }
    }

    pub(crate) fn external(&self, channel_id: u16, internal_rpc_handle: InternalRPCHandle) -> Self {
        Self {
            inner: self.inner.clone(),
            state: self.state.clone(),
            channel_closer: None,
            consumer_canceler: Some(Arc::new(ConsumerCanceler::new(
                channel_id,
                self.tag().to_string(),
                self.state.clone(),
                internal_rpc_handle,
            ))),
        }
    }

    /// Gets the consumer tag.
    ///
    /// If no consumer tag was specified when obtaining the consumer from the channel,
    /// this contains the server generated consumer tag.
    pub fn tag(&self) -> ShortString {
        self.inner.lock().tag.clone()
    }

    /// Automatically spawns the delegate on the executor for each message.
    ///
    /// Enables parallel handling of the messages.
    pub fn set_delegate<D: ConsumerDelegate + 'static>(&self, delegate: D) -> Result<()> {
        let mut inner = self.inner.lock();
        while let Some(delivery) = inner.next_delivery() {
            inner.executor.spawn(delegate.on_new_delivery(delivery))?;
        }
        inner.delegate = Some(Arc::new(Box::new(delegate)));
        Ok(())
    }

    pub(crate) fn start_new_delivery(&mut self, delivery: Delivery) {
        self.inner.lock().current_message = Some(delivery)
    }

    pub(crate) fn set_delivery_properties(&mut self, properties: BasicProperties) {
        if let Some(delivery) = self.inner.lock().current_message.as_mut() {
            delivery.properties = properties;
        }
    }

    pub(crate) fn receive_delivery_content(&mut self, payload: Vec<u8>) {
        if let Some(delivery) = self.inner.lock().current_message.as_mut() {
            delivery.receive_content(payload);
        }
    }

    pub(crate) fn new_delivery_complete(&mut self, channel: Channel) -> Result<()> {
        let mut inner = self.inner.lock();
        if let Some(delivery) = inner.current_message.take() {
            inner.new_delivery(channel, delivery)?;
        }
        Ok(())
    }

    pub(crate) fn drop_prefetched_messages(&self) -> Result<()> {
        self.inner.lock().drop_prefetched_messages()
    }

    pub(crate) fn cancel(&self) -> Result<()> {
        self.inner.lock().cancel()
    }

    pub(crate) fn set_error(&self, error: Error) -> Result<()> {
        self.inner.lock().set_error(error)
    }
}

struct ConsumerInner {
    state: Arc<Mutex<ConsumerState>>,
    current_message: Option<Delivery>,
    deliveries_in: Sender<DeliveryResult>,
    deliveries_out: Receiver<DeliveryResult>,
    task: Option<Waker>,
    tag: ShortString,
    delegate: Option<Arc<Box<dyn ConsumerDelegate>>>,
    executor: Arc<dyn Executor>,
}

pub struct ConsumerIterator {
    receiver: Receiver<DeliveryResult>,
}

impl Iterator for ConsumerIterator {
    type Item = Result<(Channel, Delivery)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok().and_then(Result::transpose)
    }
}

impl IntoIterator for Consumer {
    type Item = Result<(Channel, Delivery)>;
    type IntoIter = ConsumerIterator;

    fn into_iter(self) -> Self::IntoIter {
        ConsumerIterator {
            receiver: self.inner.lock().deliveries_out.clone(),
        }
    }
}

impl fmt::Debug for Consumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Consumer");
        if let Some(inner) = self.inner.try_lock() {
            debug
                .field("tag", &inner.tag)
                .field("executor", &inner.executor)
                .field("task", &inner.task);
        }
        if let Some(state) = self.state.try_lock() {
            debug.field("state", &*state);
        }
        debug.finish()
    }
}

impl ConsumerInner {
    fn new(
        state: Arc<Mutex<ConsumerState>>,
        consumer_tag: ShortString,
        executor: Arc<dyn Executor>,
    ) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self {
            state,
            current_message: None,
            deliveries_in: sender,
            deliveries_out: receiver,
            task: None,
            tag: consumer_tag,
            delegate: None,
            executor,
        }
    }

    fn next_delivery(&mut self) -> Option<DeliveryResult> {
        self.deliveries_out.try_recv().ok()
    }

    fn new_delivery(&mut self, channel: Channel, delivery: Delivery) -> Result<()> {
        trace!("new_delivery; consumer_tag={}", self.tag);
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor
                .spawn(delegate.on_new_delivery(Ok(Some((channel, delivery)))))?;
        } else {
            self.deliveries_in
                .send(Ok(Some((channel, delivery))))
                .expect("failed to send delivery to consumer");
        }
        if let Some(task) = self.task.as_ref() {
            task.wake_by_ref();
        }
        Ok(())
    }

    fn drop_prefetched_messages(&mut self) -> Result<()> {
        trace!("drop_prefetched_messages; consumer_tag={}", self.tag);
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor.spawn(delegate.drop_prefetched_messages())?;
        }
        while self.next_delivery().is_some() {}
        Ok(())
    }

    fn cancel(&mut self) -> Result<()> {
        trace!("cancel; consumer_tag={}", self.tag);
        let mut state = self.state.lock();
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor.spawn(delegate.on_new_delivery(Ok(None)))?;
        } else {
            self.deliveries_in
                .send(Ok(None))
                .expect("failed to send cancel to consumer");
        }
        if let Some(task) = self.task.take() {
            task.wake();
        }
        *state = ConsumerState::Canceled;
        Ok(())
    }

    fn set_error(&mut self, error: Error) -> Result<()> {
        trace!("set_error; consumer_tag={}", self.tag);
        if let Some(delegate) = self.delegate.as_ref() {
            let delegate = delegate.clone();
            self.executor.spawn(delegate.on_new_delivery(Err(error)))?;
        } else {
            self.deliveries_in
                .send(Err(error))
                .expect("failed to send error to consumer");
        }
        self.cancel()
    }
}

impl Stream for Consumer {
    type Item = Result<(Channel, Delivery)>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("consumer poll_next");
        let mut inner = self.inner.lock();
        trace!(
            "consumer poll; acquired inner lock, consumer_tag={}",
            inner.tag
        );
        inner.task = Some(cx.waker().clone());
        if let Some(delivery) = inner.next_delivery() {
            match delivery {
                Ok(Some((channel, delivery))) => {
                    trace!(
                        "delivery; channel={}, consumer_tag={}, delivery_tag={:?}",
                        channel.id(),
                        inner.tag,
                        delivery.delivery_tag
                    );
                    Poll::Ready(Some(Ok((channel, delivery))))
                }
                Ok(None) => {
                    trace!("consumer canceled; consumer_tag={}", inner.tag);
                    Poll::Ready(None)
                }
                Err(error) => Poll::Ready(Some(Err(error))),
            }
        } else {
            trace!("delivery; status=NotReady, consumer_tag={}", inner.tag);
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod futures_tests {
    use super::*;
    use crate::executor::DefaultExecutor;

    use std::task::{Context, Poll};

    use futures_test::task::new_count_waker;
    use futures_util::stream::StreamExt;

    #[test]
    fn stream_on_cancel() {
        let (waker, awoken_count) = new_count_waker();
        let mut cx = Context::from_waker(&waker);

        let mut consumer = Consumer::new(
            ShortString::from("test-consumer"),
            Arc::new(DefaultExecutor::default()),
            None,
        );

        assert_eq!(awoken_count.get(), 0);
        assert_eq!(consumer.poll_next_unpin(&mut cx), Poll::Pending);

        consumer.cancel().unwrap();

        assert_eq!(awoken_count.get(), 1);
        assert_eq!(consumer.poll_next_unpin(&mut cx), Poll::Ready(None));
    }

    #[test]
    fn stream_on_error() {
        let (waker, awoken_count) = new_count_waker();
        let mut cx = Context::from_waker(&waker);

        let mut consumer = Consumer::new(
            ShortString::from("test-consumer"),
            Arc::new(DefaultExecutor::default()),
            None,
        );

        assert_eq!(awoken_count.get(), 0);
        assert_eq!(consumer.poll_next_unpin(&mut cx), Poll::Pending);

        consumer.set_error(Error::ChannelsLimitReached).unwrap();

        assert_eq!(awoken_count.get(), 1);
        assert_eq!(
            consumer.poll_next_unpin(&mut cx),
            Poll::Ready(Some(Err(Error::ChannelsLimitReached)))
        );
    }
}
