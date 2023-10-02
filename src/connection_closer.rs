use crate::{connection_status::ConnectionStatus, internal_rpc::InternalRPCHandle, protocol};

#[derive(Clone)]
pub(crate) struct ConnectionCloser {
    status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
}

impl ConnectionCloser {
    pub(crate) fn new(status: ConnectionStatus, internal_rpc: InternalRPCHandle) -> Self {
        Self {
            status,
            internal_rpc,
        }
    }
}

impl Drop for ConnectionCloser {
    fn drop(&mut self) {
        if self.status.auto_close() {
            self.internal_rpc.close_connection(
                protocol::constants::REPLY_SUCCESS,
                "OK".to_string(),
                0,
                0,
            );
        }
    }
}
