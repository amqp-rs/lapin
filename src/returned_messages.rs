use crate::{message::BasicReturnMessage, pinky_swear::Pinky, BasicProperties, Result};
use log::error;
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
        self.inner.lock().messages.drain(..).collect()
    }

    pub(crate) fn register_pinky(&self, pinky: Pinky<Result<()>>) {
        self.inner.lock().register_pinky(pinky);
    }
}

#[derive(Debug, Default)]
pub struct Inner {
    current_message: Option<BasicReturnMessage>,
    waiting_messages: VecDeque<BasicReturnMessage>,
    messages: Vec<BasicReturnMessage>,
    pinkies: VecDeque<Pinky<Result<()>>>,
}

impl Inner {
    fn new_delivery_complete(&mut self) {
        if let Some(message) = self.current_message.take() {
            error!("Server returned us a message: {:?}", message);
            if let Some(pinky) = self.pinkies.pop_front() {
                self.notify(pinky, message);
            } else {
                self.waiting_messages.push_back(message);
            }
        }
    }

    fn register_pinky(&mut self, pinky: Pinky<Result<()>>) {
        if let Some(message) = self.waiting_messages.pop_front() {
            self.notify(pinky, message);
        } else {
            self.pinkies.push_back(pinky);
        }
    }

    fn notify(&mut self, pinky: Pinky<Result<()>>, message: BasicReturnMessage) {
        self.messages.push(message);
        pinky.swear(Ok(()));
    }
}
