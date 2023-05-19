use crate::{
    channel_receiver_state::{ChannelReceiverStates, DeliveryCause},
    types::{ChannelId, Identifier, PayloadSize},
    Result,
};
use parking_lot::Mutex;
use std::{fmt, sync::Arc};
use tracing::trace;

#[derive(Clone, Default)]
pub struct ChannelStatus(Arc<Mutex<Inner>>);

impl ChannelStatus {
    pub fn initializing(&self) -> bool {
        self.0.lock().state == ChannelState::Initial
    }

    pub fn closing(&self) -> bool {
        self.0.lock().state == ChannelState::Closing
    }

    pub fn connected(&self) -> bool {
        self.0.lock().state == ChannelState::Connected
    }

    pub(crate) fn can_receive_messages(&self) -> bool {
        [ChannelState::Closing, ChannelState::Connected].contains(&self.0.lock().state)
    }

    pub fn confirm(&self) -> bool {
        self.0.lock().confirm
    }

    pub(crate) fn set_confirm(&self) {
        self.0.lock().confirm = true;
        trace!("Publisher confirms activated");
    }

    pub fn state(&self) -> ChannelState {
        self.0.lock().state.clone()
    }

    pub(crate) fn set_state(&self, state: ChannelState) {
        self.0.lock().state = state;
    }

    pub(crate) fn auto_close(&self, id: ChannelId) -> bool {
        id != 0 && self.0.lock().state == ChannelState::Connected
    }

    #[cfg(test)]
    pub(crate) fn receiver_state(&self) -> crate::channel_receiver_state::ChannelReceiverState {
        self.0.lock().receiver_state.receiver_state()
    }

    pub(crate) fn set_will_receive(&self, class_id: Identifier, delivery_cause: DeliveryCause) {
        self.0
            .lock()
            .receiver_state
            .set_will_receive(class_id, delivery_cause);
    }

    pub(crate) fn set_content_length<
        Handler: FnOnce(&DeliveryCause, bool),
        OnInvalidClass: FnOnce(String) -> Result<()>,
        OnError: FnOnce(String) -> Result<()>,
    >(
        &self,
        channel_id: ChannelId,
        class_id: Identifier,
        length: PayloadSize,
        handler: Handler,
        invalid_class_hanlder: OnInvalidClass,
        error_handler: OnError,
    ) -> Result<()> {
        let mut inner = self.0.lock();
        let confirm_mode = inner.confirm;
        inner.receiver_state.set_content_length(
            channel_id,
            class_id,
            length,
            handler,
            invalid_class_hanlder,
            error_handler,
            confirm_mode,
        )
    }

    pub(crate) fn receive<
        Handler: FnOnce(&DeliveryCause, PayloadSize, bool),
        OnError: FnOnce(String) -> Result<()>,
    >(
        &self,
        channel_id: ChannelId,
        length: PayloadSize,
        handler: Handler,
        error_handler: OnError,
    ) -> Result<()> {
        let mut inner = self.0.lock();
        let confirm_mode = inner.confirm;
        inner
            .receiver_state
            .receive(channel_id, length, handler, error_handler, confirm_mode)
    }

    pub(crate) fn set_send_flow(&self, flow: bool) {
        self.0.lock().send_flow = flow;
    }

    pub(crate) fn flow(&self) -> bool {
        self.0.lock().send_flow
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ChannelState {
    #[default]
    Initial,
    Connected,
    Closing,
    Closed,
    Error,
}

impl fmt::Debug for ChannelStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ChannelStatus");
        if let Some(inner) = self.0.try_lock() {
            debug
                .field("state", &inner.state)
                .field("receiver_state", &inner.receiver_state)
                .field("confirm", &inner.confirm)
                .field("send_flow", &inner.send_flow);
        }
        debug.finish()
    }
}

struct Inner {
    confirm: bool,
    send_flow: bool,
    state: ChannelState,
    receiver_state: ChannelReceiverStates,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            confirm: false,
            send_flow: true,
            state: ChannelState::default(),
            receiver_state: ChannelReceiverStates::default(),
        }
    }
}
