use crate::Error;
use flume::Sender;
use futures_core::Stream;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, Weak},
    task::{Context, Poll},
};

#[derive(Clone, Debug)]
pub(crate) struct Events {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {
    listeners: Vec<Arc<Sender<Event>>>,
    /// Events not delivered yet, with the listeners registered when each
    /// was emitted.
    pending: VecDeque<(Event, Vec<Arc<Sender<Event>>>)>,
    /// An emitter is delivering `pending`.
    draining: bool,
}

struct EventStream {
    receiver: flume::r#async::RecvStream<'static, Event>,
    events: Weak<Mutex<Inner>>,
    sender: Weak<Sender<Event>>,
}

impl Stream for EventStream {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().receiver).poll_next(cx)
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        if let Some(events) = self.events.upgrade() {
            let sender = {
                let mut inner = events.lock().unwrap_or_else(|e| e.into_inner());
                inner
                    .listeners
                    .iter()
                    .position(|sender| Weak::ptr_eq(&self.sender, &Arc::downgrade(sender)))
                    .map(|index| inner.listeners.swap_remove(index))
            };
            drop(sender);
        }
    }
}

impl Events {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::default(),
        }
    }

    pub(crate) fn sender(&self) -> EventsSender {
        EventsSender(self.inner.clone())
    }

    pub(crate) fn listener(&self) -> impl Stream<Item = Event> + Send + 'static {
        let (sender, receiver) = flume::unbounded();
        let sender = Arc::new(sender);
        let stream = EventStream {
            receiver: receiver.into_stream(),
            events: Arc::downgrade(&self.inner),
            sender: Arc::downgrade(&sender),
        };
        self.lock_inner().listeners.push(sender);
        stream
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EventsSender(Arc<Mutex<Inner>>);

impl EventsSender {
    /// Events are delivered outside the registry lock, in emission order:
    /// an emitter that finds another one delivering queues its event for it,
    /// so that the IO thread and the dispatcher, or a listener waker, cannot
    /// make listeners see concurrent events in different orders.
    fn send(&self, event: Event) {
        {
            let mut inner = self.lock();
            let listeners = inner.listeners.clone();
            inner.pending.push_back((event, listeners));
            if std::mem::replace(&mut inner.draining, true) {
                return;
            }
        }
        let drainer = Drainer(self);
        while let Some((event, listeners)) = drainer.next() {
            for sender in &listeners {
                let _ = sender.send(event.clone());
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn connected(&self) {
        self.send(Event::Connected);
    }

    pub(crate) fn connection_blocked(&self, reason: String) {
        self.send(Event::ConnectionBlocked(reason));
    }

    pub(crate) fn connection_unblocked(&self) {
        self.send(Event::ConnectionUnblocked);
    }

    pub(crate) fn send_flow(&self, active: bool) {
        self.send(Event::SendFlow(active));
    }

    pub(crate) fn error(&self, error: Error) {
        self.send(Event::Error(error));
    }
}

/// Takes the pending events one by one, and gives the delivery up when none
/// is left or when a listener waker panics.
struct Drainer<'a>(&'a EventsSender);

impl Drainer<'_> {
    fn next(&self) -> Option<(Event, Vec<Arc<Sender<Event>>>)> {
        let mut inner = self.0.lock();
        let next = inner.pending.pop_front();
        if next.is_none() {
            inner.draining = false;
        }
        next
    }
}

impl Drop for Drainer<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.lock().draining = false;
        }
    }
}

/// A connection-level event delivered via [`Connection::events_listener`].
///
/// The stream produced by [`Connection::events_listener`] emits these values
/// as the connection progresses through its lifecycle.
///
/// [`Connection::events_listener`]: crate::Connection::events_listener
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Event {
    /// The connection has been established (or re-established after recovery).
    Connected,
    /// The broker has blocked the connection due to resource constraints.
    /// The inner string is the human-readable reason supplied by the broker.
    ConnectionBlocked(String),
    /// The broker has unblocked the connection.
    ConnectionUnblocked,
    /// The broker has changed the allowed flow direction.
    /// `true` means publishing is permitted; `false` means it is paused.
    SendFlow(bool),
    /// An error occurred on the connection.
    Error(Error),
}

#[cfg(test)]
mod tests {
    use super::{Event, Events};

    use futures_lite::{StreamExt, future::block_on};
    use std::{
        future::Future,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Wake, Waker},
    };

    #[test]
    fn events_reach_every_listener() {
        let events = Events::new();
        let mut first = events.listener();
        let mut second = events.listener();
        events.sender().connected();

        assert!(matches!(block_on(first.next()), Some(Event::Connected)));
        assert!(matches!(block_on(second.next()), Some(Event::Connected)));
    }

    #[test]
    fn dropped_listeners_are_pruned_immediately() {
        let events = Events::new();
        for _ in 0..100 {
            drop(events.listener());
            assert!(events.lock_inner().listeners.is_empty());
        }
    }

    #[test]
    fn listeners_end_after_all_event_producers_are_dropped() {
        let events = Events::new();
        let sender = events.sender();
        let mut listener = events.listener();
        sender.connected();
        drop(events);

        assert!(matches!(block_on(listener.next()), Some(Event::Connected)));
        let mut next = std::pin::pin!(listener.next());
        let mut cx = Context::from_waker(Waker::noop());
        assert!(next.as_mut().poll(&mut cx).is_pending());
        drop(sender);
        assert!(matches!(
            next.as_mut().poll(&mut cx),
            std::task::Poll::Ready(None)
        ));
    }

    #[test]
    fn every_listener_sees_events_in_emission_order() {
        struct EmitOnWake {
            events: Events,
            emitted: AtomicBool,
        }

        impl Wake for EmitOnWake {
            fn wake(self: Arc<Self>) {
                if !self.emitted.swap(true, Ordering::SeqCst) {
                    self.events.sender().connection_unblocked();
                }
            }
        }

        // The listener channel wakes its receiver under its own lock, so an
        // emission straight from the waker would never return.
        let (done, finished) = std::sync::mpsc::channel();
        let test = std::thread::spawn(move || {
            let events = Events::new();
            let mut first = events.listener();
            let mut second = events.listener();
            let emit_on_wake = Arc::new(EmitOnWake {
                events: events.clone(),
                emitted: AtomicBool::new(false),
            });
            let waker = Waker::from(emit_on_wake.clone());
            {
                let mut next = std::pin::pin!(first.next());
                assert!(
                    next.as_mut()
                        .poll(&mut Context::from_waker(&waker))
                        .is_pending()
                );
            }
            // Delivering this to the first listener emits another event.
            events.sender().connection_blocked("memory".into());
            assert!(emit_on_wake.emitted.load(Ordering::SeqCst));
            for listener in [&mut first, &mut second] {
                assert!(matches!(
                    block_on(listener.next()),
                    Some(Event::ConnectionBlocked(_))
                ));
                assert!(matches!(
                    block_on(listener.next()),
                    Some(Event::ConnectionUnblocked)
                ));
            }
            done.send(()).unwrap();
        });
        assert!(
            !matches!(
                finished.recv_timeout(std::time::Duration::from_secs(5)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ),
            "event delivery deadlocked"
        );
        test.join().unwrap();
    }

    #[test]
    fn event_waker_can_register_and_drop_another_listener() {
        struct RegisterOnWake {
            events: Events,
            called: AtomicBool,
        }

        impl Wake for RegisterOnWake {
            fn wake(self: Arc<Self>) {
                drop(self.events.listener());
                self.called.store(true, Ordering::SeqCst);
            }
        }

        for drop_listener in [false, true] {
            let events = Events::new();
            let mut listener = events.listener();
            let register_on_wake = Arc::new(RegisterOnWake {
                events: events.clone(),
                called: AtomicBool::new(false),
            });
            let waker = Waker::from(register_on_wake.clone());
            let mut cx = Context::from_waker(&waker);
            {
                let mut next = std::pin::pin!(listener.next());
                assert!(next.as_mut().poll(&mut cx).is_pending());
            }

            if drop_listener {
                drop(listener);
                assert!(events.lock_inner().listeners.is_empty());
            } else {
                events.sender().connected();
                assert_eq!(events.lock_inner().listeners.len(), 1);
                assert!(matches!(block_on(listener.next()), Some(Event::Connected)));
            }
            assert!(register_on_wake.called.load(Ordering::SeqCst));
        }
    }
}
