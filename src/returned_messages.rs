use crate::{
    message::BasicReturnMessage,
    pinky_swear::{Pinky, PinkyBroadcaster},
    BasicProperties, PublisherConfirm, Result,
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

    pub(crate) fn register_pinky(&self, pinky: (Pinky<Result<PublisherConfirm>>, PinkyBroadcaster<Result<PublisherConfirm>>)) {
        self.inner.lock().pinkies.push_back(pinky);
    }
}

#[derive(Debug, Default)]
pub struct Inner {
    current_message: Option<BasicReturnMessage>,
    messages: Vec<BasicReturnMessage>,
    pinkies: VecDeque<(Pinky<Result<PublisherConfirm>>, PinkyBroadcaster<Result<PublisherConfirm>>)>,
}

impl Inner {
    fn new_delivery_complete(&mut self) {
        if let Some(message) = self.current_message.take() {
            error!("Server returned us a message: {:?}", message);
            self.messages.push(message);
            if let Some((pinky, _broadcaster)) = self.pinkies.pop_front() {
                pinky.swear(Ok(PublisherConfirm::Nack));
            }
        }
    }
}
