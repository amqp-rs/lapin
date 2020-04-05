use crate::{
    message::BasicReturnMessage, promises::Promises, publisher_confirm::Confirmation,
    BasicProperties, Promise,
};
use log::{trace, warn};
use parking_lot::Mutex;
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone, Debug, Default)]
pub(crate) struct ReturnedMessages {
    inner: Arc<Mutex<Inner>>,
}

impl ReturnedMessages {
    pub(crate) fn start_new_delivery(&self, message: BasicReturnMessage) {
        self.inner.lock().current_message = Some(message);
    }

    pub(crate) fn set_delivery_properties(&self, properties: BasicProperties) {
        if let Some(message) = self.inner.lock().current_message.as_mut() {
            message.delivery.properties = properties;
        }
    }

    pub(crate) fn new_delivery_complete(&self, confirm_mode: bool) {
        self.inner.lock().new_delivery_complete(confirm_mode);
    }

    pub(crate) fn receive_delivery_content(&self, data: Vec<u8>) {
        if let Some(message) = self.inner.lock().current_message.as_mut() {
            message.delivery.data.extend(data);
        }
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

#[derive(Debug, Default)]
pub struct Inner {
    current_message: Option<BasicReturnMessage>,
    non_confirm_messages: Vec<BasicReturnMessage>,
    waiting_messages: VecDeque<BasicReturnMessage>,
    messages: Vec<BasicReturnMessage>,
    dropped_confirms: Promises<Confirmation>,
}

impl Inner {
    fn new_delivery_complete(&mut self, confirm_mode: bool) {
        if let Some(message) = self.current_message.take() {
            warn!("Server returned us a message: {:?}", message);
            if confirm_mode {
                self.waiting_messages.push_back(message);
            } else {
                self.non_confirm_messages.push(message);
            }
        }
    }

    fn register_dropped_confirm(&mut self, promise: Promise<Confirmation>) {
        if let Some(confirmation) = self.dropped_confirms.register(promise) {
            if let Ok(confirmation) = confirmation {
                if let Some(message) = confirmation.take_message() {
                    trace!("Dropped PublisherConfirm was carrying a message, storing it");
                    self.messages.push(message);
                    return;
                }
            }
            trace!("Dropped PublisherConfirm was ready but didn't carry a message, discarding");
        } else {
            trace!("Storing dropped PublisherConfirm for further use");
        }
    }

    fn drain(&mut self) -> Vec<BasicReturnMessage> {
        let mut messages = std::mem::take(&mut self.messages);
        if !self.non_confirm_messages.is_empty() {
            let mut non_confirm_messages = std::mem::take(&mut self.non_confirm_messages);
            non_confirm_messages.append(&mut messages);
            messages = non_confirm_messages;
        }
        if let Some(confirmations) = self.dropped_confirms.try_wait() {
            for confirmation in confirmations {
                if let Ok(confirmation) = confirmation {
                    if let Some(message) = confirmation.take_message() {
                        trace!("PublisherConfirm was carrying a message, storing it");
                        messages.push(message);
                        continue;
                    }
                }
                trace!("PublisherConfirm was ready but didn't carry a message, discarding");
            }
        }
        messages
    }
}
