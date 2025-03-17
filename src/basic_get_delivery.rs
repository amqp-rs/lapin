use crate::{
    message::BasicGetMessage,
    options::BasicGetOptions,
    topology_internal::BasicGetDefinitionInternal,
    types::{PayloadSize, ShortString},
    BasicProperties, PromiseResolver,
};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Default)]
pub(crate) struct BasicGetDelivery(Arc<Mutex<Inner>>);

impl BasicGetDelivery {
    pub(crate) fn start_new_delivery(
        &self,
        queue: ShortString,
        options: BasicGetOptions,
        message: BasicGetMessage,
        resolver: PromiseResolver<Option<BasicGetMessage>>,
    ) {
        self.lock_inner()
            .start_new_delivery(queue, options, message, resolver);
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

    pub(crate) fn recover(&self) -> Option<BasicGetDefinitionInternal> {
        self.lock_inner()
            .0
            .take()
            .map(|inner| BasicGetDefinitionInternal {
                queue: inner.queue,
                options: inner.options,
                resolver: inner.resolver,
            })
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
        queue: ShortString,
        options: BasicGetOptions,
        message: BasicGetMessage,
        resolver: PromiseResolver<Option<BasicGetMessage>>,
    ) {
        self.0 = Some(InnerData {
            queue,
            options,
            message,
            resolver,
        });
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
    queue: ShortString,
    options: BasicGetOptions,
    message: BasicGetMessage,
    resolver: PromiseResolver<Option<BasicGetMessage>>,
}
