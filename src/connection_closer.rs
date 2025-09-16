use crate::{ConnectionStatus, internal_rpc::InternalRPCHandle, protocol};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub(crate) struct ConnectionCloser {
    status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
    noop: AtomicBool,
}

impl ConnectionCloser {
    pub(crate) fn new(status: ConnectionStatus, internal_rpc: InternalRPCHandle) -> Self {
        Self {
            status,
            internal_rpc,
            noop: AtomicBool::new(false),
        }
    }

    pub(crate) fn noop(&self) {
        self.noop.store(true, Ordering::SeqCst);
    }
}

impl Drop for ConnectionCloser {
    fn drop(&mut self) {
        if self.noop.load(Ordering::SeqCst) {
            return;
        }

        if self.status.auto_close() {
            self.internal_rpc.close_connection(
                protocol::constants::REPLY_SUCCESS,
                "OK".into(),
                0,
                0,
            );
        }
    }
}
