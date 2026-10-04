use crate::{ConnectionState, ConnectionStatus, ErrorKind, internal_rpc::InternalRPCHandle};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub(crate) struct ConnectionCloser {
    status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
    noop: AtomicBool,
    active: AtomicBool,
}

impl ConnectionCloser {
    pub(crate) fn new(status: ConnectionStatus, internal_rpc: InternalRPCHandle) -> Self {
        Self {
            status,
            internal_rpc,
            noop: AtomicBool::new(false),
            active: AtomicBool::new(false),
        }
    }

    pub(crate) fn noop(&self) {
        self.noop.store(true, Ordering::SeqCst);
    }

    pub(crate) fn activate(&self) {
        self.active.store(true, Ordering::SeqCst);
    }
}

impl Drop for ConnectionCloser {
    fn drop(&mut self) {
        if self.noop.load(Ordering::SeqCst) || !self.active.load(Ordering::SeqCst) {
            return;
        }

        if self.status.reconnecting() {
            self.status.set_state(ConnectionState::Closed);
            self.internal_rpc.set_connection_closed(
                ErrorKind::InvalidConnectionState(ConnectionState::Closed).into(),
            );
        } else if self.status.auto_close() {
            // Recovery may start before this close is handled: the
            // dispatcher checks the state again.
            self.internal_rpc.close_dropped_connection();
        }
    }
}
