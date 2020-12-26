use crate::{
    message::BasicReturnMessage, publisher_confirm::Confirmation, types::PayloadSize,
    BasicProperties, Promise,
};
use parking_lot::Mutex;
use std::{collections::VecDeque, fmt, sync::Arc};
use tracing::{trace, warn};

#[derive(Clone, Default)]
pub(crate) struct ReturnedMessages {
    inner: Arc<Mutex<Inner>>,
}

impl ReturnedMessages {
    pub(crate) fn start_new_delivery(&self, message: BasicReturnMessage) {
        self.inner.lock().current_message = Some(message);
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        size: PayloadSize,
        properties: BasicProperties,
        confirm_mode: bool,
    ) {
        self.inner
            .lock()
            .handle_content_header_frame(size, properties, confirm_mode);
    }

    pub(crate) fn handle_body_frame(
        &self,
        remaining_size: PayloadSize,
        payload: Vec<u8>,
        confirm_mode: bool,
    ) {
        self.inner
            .lock()
            .handle_body_frame(remaining_size, payload, confirm_mode);
    }

    pub(crate) fn drain(&self) -> Vec<BasicReturnMessage> {
        self.inner.lock().drain()
    }

    pub(crate) fn register_dropped_confirm(&self, promise: Promise<Confirmation>) {
        self.inner.lock().register_dropped_confirm(promise);
    }

    pub(crate) fn get_waiting_message(&self) -> Option<BasicReturnMessage> {
        self.inner.lock().waiting_messages.pop_front()
    }
}

impl fmt::Debug for ReturnedMessages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ReturnedMessages");
        if let Some(inner) = self.inner.try_lock() {
            debug
                .field("waiting_messages", &inner.waiting_messages)
                .field("messages", &inner.messages)
                .field("non_confirm_messages", &inner.non_confirm_messages);
        }
        debug.finish()
    }
}

#[derive(Default)]
pub struct Inner {
    current_message: Option<BasicReturnMessage>,
    non_confirm_messages: Vec<BasicReturnMessage>,
    waiting_messages: VecDeque<BasicReturnMessage>,
    messages: Vec<BasicReturnMessage>,
    dropped_confirms: Vec<Promise<Confirmation>>,
}

impl Inner {
    fn handle_content_header_frame(
        &mut self,
        size: PayloadSize,
        properties: BasicProperties,
        confirm_mode: bool,
    ) {
        if let Some(message) = self.current_message.as_mut() {
            message.properties = properties;
        }
        if size == 0 {
            self.new_delivery_complete(confirm_mode);
        }
    }

    fn handle_body_frame(
        &mut self,
        remaining_size: PayloadSize,
        payload: Vec<u8>,
        confirm_mode: bool,
    ) {
        if let Some(message) = self.current_message.as_mut() {
            message.receive_content(payload);
        }
        if remaining_size == 0 {
            self.new_delivery_complete(confirm_mode);
        }
    }

    fn new_delivery_complete(&mut self, confirm_mode: bool) {
        if let Some(message) = self.current_message.take() {
            warn!(?message, "Server returned us a message");
            if confirm_mode {
                self.waiting_messages.push_back(message);
            } else {
                self.non_confirm_messages.push(message);
            }
        }
    }

    fn register_dropped_confirm(&mut self, promise: Promise<Confirmation>) {
        if let Some(confirmation) = promise.try_wait() {
            if let Ok(Confirmation::Nack(Some(message))) | Ok(Confirmation::Ack(Some(message))) =
                confirmation
            {
                trace!("Dropped PublisherConfirm was carrying a message, storing it");
                self.messages.push(*message);
            } else {
                trace!("Dropped PublisherConfirm was ready but didn't carry a message, discarding");
            }
        } else {
            trace!("Storing dropped PublisherConfirm for further use");
            self.dropped_confirms.push(promise);
        }
    }

    fn drain(&mut self) -> Vec<BasicReturnMessage> {
        let mut messages = std::mem::take(&mut self.messages);
        if !self.non_confirm_messages.is_empty() {
            let mut non_confirm_messages = std::mem::take(&mut self.non_confirm_messages);
            non_confirm_messages.append(&mut messages);
            messages = non_confirm_messages;
        }
        let before = self.dropped_confirms.len();
        if before != 0 {
            for promise in std::mem::take(&mut self.dropped_confirms) {
                if let Some(confirmation) = promise.try_wait() {
                    if let Ok(Confirmation::Nack(Some(message)))
                    | Ok(Confirmation::Ack(Some(message))) = confirmation
                    {
                        trace!("PublisherConfirm was carrying a message, storing it");
                        messages.push(*message);
                    } else {
                        trace!("PublisherConfirm was ready but didn't carry a message, discarding");
                    }
                } else {
                    trace!("PublisherConfirm wasn't ready yet, storing it back");
                    self.dropped_confirms.push(promise);
                }
            }
            trace!(
                %before,
                after=%self.dropped_confirms.len(),
                "PublisherConfirms processed"
            );
        }
        messages
    }
}
