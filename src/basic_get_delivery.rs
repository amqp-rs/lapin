use crate::{BasicProperties, PromiseResolver, message::BasicGetMessage, types::PayloadSize};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Default)]
pub(crate) struct BasicGetDelivery(Arc<Mutex<Inner>>);

impl BasicGetDelivery {
    pub(crate) fn start_new_delivery(
        &self,
        message: BasicGetMessage,
        resolver: PromiseResolver<Option<BasicGetMessage>>,
    ) {
        self.lock_inner().start_new_delivery(message, resolver);
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        size: PayloadSize,
        properties: BasicProperties,
    ) {
        self.lock_inner()
            .handle_content_header_frame(size, properties);
    }

    pub(crate) fn handle_body_frame(&self, remaining_size: PayloadSize, payload: Vec<u8>) {
        self.lock_inner().handle_body_frame(remaining_size, payload);
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for BasicGetDelivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BasicGetDelivery").finish()
    }
}

#[derive(Default)]
struct Inner(Option<InnerData>);

impl Inner {
    fn start_new_delivery(
        &mut self,
        message: BasicGetMessage,
        resolver: PromiseResolver<Option<BasicGetMessage>>,
    ) {
        self.0 = Some(InnerData { message, resolver });
    }

    fn handle_content_header_frame(&mut self, size: PayloadSize, properties: BasicProperties) {
        if let Some(inner) = self.0.as_mut() {
            inner.message.properties = properties;
        }
        if size == 0 {
            self.new_delivery_complete();
        }
    }

    fn handle_body_frame(&mut self, remaining_size: PayloadSize, payload: Vec<u8>) {
        if let Some(inner) = self.0.as_mut() {
            inner.message.receive_content(payload);
        }
        if remaining_size == 0 {
            self.new_delivery_complete();
        }
    }

    fn new_delivery_complete(&mut self) {
        if let Some(inner) = self.0.take() {
            inner.resolver.resolve(Some(inner.message));
        }
    }
}

struct InnerData {
    message: BasicGetMessage,
    resolver: PromiseResolver<Option<BasicGetMessage>>,
}
