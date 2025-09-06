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
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use tracing::trace;

#[derive(Clone)]
pub struct ChannelStatus(Arc<RwLock<Inner>>);

impl ChannelStatus {
    pub(crate) fn new(id: ChannelId, internal_rpc: InternalRPCHandle) -> Self {
        Self(Arc::new(RwLock::new(Inner::new(id, internal_rpc))))
    }

    pub fn initializing(&self) -> bool {
        [ChannelState::Initial, ChannelState::Reconnecting].contains(&self.read().state)
    }

    pub fn closing(&self) -> bool {
        [ChannelState::Closing, ChannelState::Reconnecting].contains(&self.read().state)
    }

    pub fn connected(&self) -> bool {
        self.read().state == ChannelState::Connected
    }

    pub fn reconnecting(&self) -> bool {
        self.read().state == ChannelState::Reconnecting
    }

    pub(crate) fn connected_or_recovering(&self) -> bool {
        [ChannelState::Connected, ChannelState::Reconnecting].contains(&self.read().state)
    }

    pub(crate) fn update_recovery_context<R, F: Fn(&mut ChannelRecoveryContext) -> R>(
        &self,
        apply: F,
    ) -> Option<R> {
        Some(apply(self.write().recovery_context.as_mut()?))
    }

    pub(crate) fn finalize_connection(&self) {
        self.write().finalize_connection();
    }

    pub(crate) fn can_receive_messages(&self) -> bool {
        [
            ChannelState::Closing,
            ChannelState::Connected,
            ChannelState::Reconnecting,
        ]
        .contains(&self.read().state)
    }

    pub fn confirm(&self) -> bool {
        self.read().confirm
    }

    pub(crate) fn set_confirm(&self) {
        let mut inner = self.write();
        inner.confirm = true;
        trace!("Publisher confirms activated");
        inner.finalize_connection();
    }

    pub(crate) fn set_state(&self, state: ChannelState) {
        self.write().state = state;
    }

    pub(crate) fn state_error(&self, context: &'static str) -> Error {
        let inner = self.read();
        let error = Error::from(ErrorKind::InvalidChannelState(inner.state, context));
        if inner.state == ChannelState::Reconnecting {
            return error.with_notifier(inner.notifier());
        }
        error
    }

    pub(crate) fn set_reconnecting(&self, error: Error, topology: ChannelDefinition) -> Error {
        self.write().set_reconnecting(error, topology)
    }

    pub(crate) fn auto_close(&self, id: ChannelId) -> bool {
        id != 0 && self.connected()
    }

    #[cfg(test)]
    pub(crate) fn receiver_state(&self) -> crate::channel_receiver_state::ChannelReceiverState {
        self.write().receiver_state.receiver_state()
    }

    pub(crate) fn set_will_receive(
        &self,
        class_id: Identifier,
        delivery_cause: DeliveryCause,
    ) -> KillSwitch {
        let mut inner = self.write();
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
        let mut inner = self.write();
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
        let mut inner = self.write();
        let confirm_mode = inner.confirm;
        inner
            .receiver_state
            .receive(channel_id, length, handler, error_handler, confirm_mode)
    }

    pub(crate) fn set_send_flow(&self, flow: bool) {
        self.write().send_flow = flow;
    }

    pub(crate) fn flow(&self) -> bool {
        self.read().send_flow
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.0.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.0.write().unwrap_or_else(|e| e.into_inner())
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
        if let Ok(inner) = self.0.try_read() {
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
