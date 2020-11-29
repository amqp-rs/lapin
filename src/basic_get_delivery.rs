use crate::{
    message::BasicGetMessage, options::BasicGetOptions,
    topology_internal::BasicGetDefinitionInternal, types::ShortString, BasicProperties,
    PayloadSize, PromiseResolver,
};
use parking_lot::Mutex;
use std::{fmt, sync::Arc};

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
        self.0
            .lock()
            .start_new_delivery(queue, options, message, resolver);
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        size: PayloadSize,
        properties: BasicProperties,
    ) {
        self.0.lock().handle_content_header_frame(size, properties);
    }

    pub(crate) fn handle_body_frame(&self, remaining_size: PayloadSize, payload: Vec<u8>) {
        self.0.lock().handle_body_frame(remaining_size, payload);
    }

    pub(crate) fn recover(&self) -> Option<BasicGetDefinitionInternal> {
        self.0
            .lock()
            .0
            .take()
            .map(|inner| BasicGetDefinitionInternal {
                queue: inner.queue,
                options: inner.options,
                resolver: inner.resolver,
            })
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
            inner.resolver.swear(Ok(Some(inner.message)));
        }
    }
}

struct InnerData {
    queue: ShortString,
    options: BasicGetOptions,
    message: BasicGetMessage,
    resolver: PromiseResolver<Option<BasicGetMessage>>,
}
