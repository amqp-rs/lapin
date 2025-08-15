use crate::{
    Error, ErrorKind, Result,
    channel_receiver_state::{ChannelReceiverStates, DeliveryCause},
    channel_recovery_context::ChannelRecoveryContext,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    notifier::Notifier,
    topology::ChannelDefinition,
    types::{ChannelId, Identifier, PayloadSize},
};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};
use tracing::trace;

#[derive(Clone)]
pub struct ChannelStatus(Arc<Mutex<Inner>>);

impl ChannelStatus {
    pub(crate) fn new(id: ChannelId, internal_rpc: InternalRPCHandle) -> Self {
        Self(Arc::new(Mutex::new(Inner::new(id, internal_rpc))))
    }

    pub fn initializing(&self) -> bool {
        [ChannelState::Initial, ChannelState::Reconnecting].contains(&self.lock_inner().state)
    }

    pub fn closing(&self) -> bool {
        [ChannelState::Closing, ChannelState::Reconnecting].contains(&self.lock_inner().state)
    }

    pub fn connected(&self) -> bool {
        self.lock_inner().state == ChannelState::Connected
    }

    pub fn reconnecting(&self) -> bool {
        self.lock_inner().state == ChannelState::Reconnecting
    }

    pub(crate) fn connected_or_recovering(&self) -> bool {
        [ChannelState::Connected, ChannelState::Reconnecting].contains(&self.lock_inner().state)
    }

    pub(crate) fn update_recovery_context<R, F: Fn(&mut ChannelRecoveryContext) -> R>(
        &self,
        apply: F,
    ) -> Option<R> {
        Some(apply(self.lock_inner().recovery_context.as_mut()?))
    }

    pub(crate) fn finalize_connection(&self) {
        self.lock_inner().finalize_connection();
    }

    pub(crate) fn can_receive_messages(&self) -> bool {
        [
            ChannelState::Closing,
            ChannelState::Connected,
            ChannelState::Reconnecting,
        ]
        .contains(&self.lock_inner().state)
    }

    pub fn confirm(&self) -> bool {
        self.lock_inner().confirm
    }

    pub(crate) fn set_confirm(&self) {
        let mut inner = self.lock_inner();
        inner.confirm = true;
        trace!("Publisher confirms activated");
        inner.finalize_connection();
    }

    pub(crate) fn set_state(&self, state: ChannelState) {
        self.lock_inner().state = state;
    }

    pub(crate) fn state_error(&self, context: &'static str) -> Error {
        let inner = self.lock_inner();
        let error = Error::from(ErrorKind::InvalidChannelState(inner.state, context));
        if inner.state == ChannelState::Reconnecting {
            return error.with_notifier(inner.notifier());
        }
        error
    }

    pub(crate) fn set_reconnecting(&self, error: Error, topology: ChannelDefinition) -> Error {
        self.lock_inner().set_reconnecting(error, topology)
    }

    pub(crate) fn auto_close(&self, id: ChannelId) -> bool {
        id != 0 && self.lock_inner().state == ChannelState::Connected
    }

    #[cfg(test)]
    pub(crate) fn receiver_state(&self) -> crate::channel_receiver_state::ChannelReceiverState {
        self.lock_inner().receiver_state.receiver_state()
    }

    pub(crate) fn set_will_receive(
        &self,
        class_id: Identifier,
        delivery_cause: DeliveryCause,
    ) -> KillSwitch {
        let mut inner = self.lock_inner();
        inner
            .receiver_state
            .set_will_receive(class_id, delivery_cause);
        inner.killswitch.clone()
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
        let mut inner = self.lock_inner();
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
        let mut inner = self.lock_inner();
        let confirm_mode = inner.confirm;
        inner
            .receiver_state
            .receive(channel_id, length, handler, error_handler, confirm_mode)
    }

    pub(crate) fn set_send_flow(&self, flow: bool) {
        self.lock_inner().send_flow = flow;
    }

    pub(crate) fn flow(&self) -> bool {
        self.lock_inner().send_flow
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelState {
    #[default]
    Initial,
    Reconnecting,
    Connected,
    Closing,
    Closed,
    Error,
}

impl fmt::Debug for ChannelStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ChannelStatus");
        if let Ok(inner) = self.0.try_lock() {
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
    id: ChannelId,
    confirm: bool,
    send_flow: bool,
    state: ChannelState,
    receiver_state: ChannelReceiverStates,
    recovery_context: Option<ChannelRecoveryContext>,
    killswitch: KillSwitch,
    internal_rpc: InternalRPCHandle,
}

impl Inner {
    fn new(id: ChannelId, internal_rpc: InternalRPCHandle) -> Self {
        let this = Self {
            id,
            confirm: false,
            send_flow: true,
            state: ChannelState::default(),
            receiver_state: ChannelReceiverStates::default(),
            recovery_context: None,
            killswitch: KillSwitch::default(),
            internal_rpc,
        };
        this.update_rpc_status();
        this
    }

    fn update_rpc_status(&self) {
        self.internal_rpc
            .set_channel_status(self.id, self.killswitch.clone());
    }

    fn set_reconnecting(&mut self, error: Error, topology: ChannelDefinition) -> Error {
        self.state = ChannelState::Reconnecting;
        std::mem::take(&mut self.killswitch).kill();
        self.receiver_state.reset();
        let ctx = ChannelRecoveryContext::new(error, topology);
        let error = ctx.cause();
        self.recovery_context = Some(ctx);
        error
    }

    pub(crate) fn finalize_connection(&mut self) {
        self.state = ChannelState::Connected;
        self.update_rpc_status();
        if let Some(ctx) = self.recovery_context.take() {
            ctx.finalize_recovery();
        }
    }

    fn notifier(&self) -> Option<Notifier> {
        Some(self.recovery_context.as_ref()?.notifier())
    }
}
