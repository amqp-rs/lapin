use crate::Error;
use flume::{self, Receiver, Sender};
use futures_core::Stream;
use std::sync::Arc;

#[derive(Clone, Debug)]
// Wrap in an arc not to temper with receiver count
pub(crate) struct Events(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    sender: Sender<Event>,
    receiver: Receiver<Event>,
}

impl Events {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self(Arc::new(Inner { sender, receiver }))
    }

    pub(crate) fn sender(&self) -> EventsSender {
        EventsSender(self.0.sender.clone())
    }

    pub(crate) fn listener(&self) -> impl Stream<Item = Event> + Send + 'static {
        self.0.receiver.clone().into_stream()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EventsSender(Sender<Event>);

impl EventsSender {
    fn send(&self, event: Event) {
        // Do nothing if we don't have at least one external receiver
        if self.0.receiver_count() > 1 {
            // The only possibility of error is if we have several external receivers and the
            // connection was already dropped, so we can safely ignore this.
            let _ = self.0.send(event);
        }
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

    pub(crate) fn error(&self, error: Error) {
        self.send(Event::Error(error));
    }
}

/// An event happening on the connection
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Event {
    Connected,
    ConnectionBlocked(String),
    ConnectionUnblocked,
    Error(Error),
}
