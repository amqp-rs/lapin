use crate::{
    message::BasicReturnMessage, promises::Promises, publisher_confirm::Confirmation,
    BasicProperties, ConfirmationBroadcaster, Promise,
};
use log::{error, trace};
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

    pub(crate) fn register_resolver(&self, resolver: ConfirmationBroadcaster) {
        self.inner.lock().register_resolver(resolver);
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
    pinkies: VecDeque<ConfirmationBroadcaster>,
}

impl Inner {
    fn register_resolver(&mut self, resolver: ConfirmationBroadcaster) {
        if let Some(message) = self.waiting_messages.pop_front() {
            resolver.swear(Ok(Confirmation::Nack(Box::new(message))));
        } else {
            self.pinkies.push_back(resolver);
        }
    }

    fn register_dropped_confirm(&mut self, promise: Promise<Confirmation>) {
        if let Some(confirmation) = self.dropped_confirms.register(promise) {
            if let Ok(Confirmation::Nack(message)) = confirmation {
                trace!("Dropped PublisherConfirm was a Nack, storing message");
                self.messages.push(*message);
            } else {
                trace!("Dropped PublisherConfirm was ready but not a Nack, discarding");
            }
        } else {
            trace!("Storing dropped PublisherConfirm for further use");
        }
    }

    fn new_delivery_complete(&mut self, confirm_mode: bool) {
        if let Some(message) = self.current_message.take() {
            error!("Server returned us a message: {:?}", message);
            if confirm_mode {
                if let Some(resolver) = self.pinkies.pop_front() {
                    resolver.swear(Ok(Confirmation::Nack(Box::new(message))));
                } else {
                    self.waiting_messages.push_back(message);
                }
            } else {
                self.non_confirm_messages.push(message);
            }
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
                if let Ok(Confirmation::Nack(message)) = confirmation {
                    trace!("PublisherConfirm was a Nack, storing message");
                    messages.push(*message);
                } else {
                    trace!("PublisherConfirm was ready but not a Nack, discarding");
                }
            }
        }
        messages
    }
}
