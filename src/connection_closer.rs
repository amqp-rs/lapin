use crate::{connection_status::ConnectionStatus, internal_rpc::InternalRPCHandle, protocol};

pub(crate) struct ConnectionCloser {
    status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
    noop: bool,
}

impl ConnectionCloser {
    pub(crate) fn new(
        status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        noop: bool,
    ) -> Self {
        Self {
            status,
            internal_rpc,
            noop,
        }
    }
}

impl Drop for ConnectionCloser {
    fn drop(&mut self) {
        if self.noop {
            return;
        }

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
