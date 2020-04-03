use crate::{
    message::BasicReturnMessage,
    pinky_swear::{PinkyBroadcaster, PinkySwear},
    promises::Promises,
    publisher_confirm::Confirmation,
    BasicProperties, Result,
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

    pub(crate) fn new_delivery_complete(&self) {
        self.inner.lock().new_delivery_complete();
    }

    pub(crate) fn receive_delivery_content(&self, data: Vec<u8>) {
        if let Some(message) = self.inner.lock().current_message.as_mut() {
            message.delivery.data.extend(data);
        }
    }

    pub(crate) fn drain(&self) -> Vec<BasicReturnMessage> {
        self.inner.lock().drain()
    }

    pub(crate) fn register_pinky(&self, pinky: PinkyBroadcaster<Result<Confirmation>>) {
        self.inner.lock().register_pinky(pinky);
    }

    pub(crate) fn register_dropped_confirm(&self, promise: PinkySwear<Result<Confirmation>>) {
        self.inner.lock().register_dropped_confirm(promise);
    }
}

#[derive(Debug, Default)]
pub struct Inner {
    current_message: Option<BasicReturnMessage>,
    waiting_messages: VecDeque<BasicReturnMessage>,
    messages: Vec<BasicReturnMessage>,
    dropped_confirms: Promises<Result<Confirmation>>,
    pinkies: VecDeque<PinkyBroadcaster<Result<Confirmation>>>,
}

impl Inner {
    fn register_pinky(&mut self, pinky: PinkyBroadcaster<Result<Confirmation>>) {
        if let Some(message) = self.waiting_messages.pop_front() {
            pinky.swear(Ok(Confirmation::Nack(Box::new(message))));
        } else {
            self.pinkies.push_back(pinky);
        }
    }

    fn register_dropped_confirm(&mut self, promise: PinkySwear<Result<Confirmation>>) {
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

    fn new_delivery_complete(&mut self) {
        if let Some(message) = self.current_message.take() {
            error!("Server returned us a message: {:?}", message);
            if let Some(pinky) = self.pinkies.pop_front() {
                pinky.swear(Ok(Confirmation::Nack(Box::new(message))));
            } else {
                self.waiting_messages.push_back(message);
            }
        }
    }

    fn drain(&mut self) -> Vec<BasicReturnMessage> {
        let mut messages = std::mem::take(&mut self.messages);
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
