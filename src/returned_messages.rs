use crate::{
    message::BasicReturnMessage,
    pinky_swear::{Pinky, PinkyBroadcaster},
    BasicProperties, Confirmation,
};
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

    pub(crate) fn register_pinky(
        &self,
        pinky: (Pinky<Confirmation>, PinkyBroadcaster<Confirmation>),
    ) {
        self.inner.lock().register_pinky(pinky);
    }
}

#[derive(Debug, Default)]
pub struct Inner {
    current_message: Option<BasicReturnMessage>,
    new_messages: VecDeque<BasicReturnMessage>,
    messages: Vec<BasicReturnMessage>,
    pinkies: VecDeque<(Pinky<Confirmation>, PinkyBroadcaster<Confirmation>)>,
}

impl Inner {
    fn register_pinky(&mut self, pinky: (Pinky<Confirmation>, PinkyBroadcaster<Confirmation>)) {
        if let Some(message) = self.new_messages.pop_front() {
            self.forward_message(pinky.0, message);
        } else {
            self.pinkies.push_back(pinky);
        }
    }

    fn new_delivery_complete(&mut self) {
        if let Some(message) = self.current_message.take() {
            error!("Server returned us a message: {:?}", message);
            if let Some((pinky, _broadcaster)) = self.pinkies.pop_front() {
                self.forward_message(pinky, message);
            } else {
                self.new_messages.push_back(message);
            }
        }
    }

    fn forward_message(&mut self, pinky: Pinky<Confirmation>, message: BasicReturnMessage) {
        self.messages.push(message.clone()); // FIXME: drop message if the Nack is consumed?
        pinky.swear(Confirmation::Nack(message));
    }
}
