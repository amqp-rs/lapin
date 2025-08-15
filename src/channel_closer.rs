use crate::{ChannelStatus, internal_rpc::InternalRPCHandle, protocol, types::ChannelId};
use std::fmt;

pub(crate) struct ChannelCloser {
    id: ChannelId,
    status: ChannelStatus,
    internal_rpc: InternalRPCHandle,
}

impl ChannelCloser {
    pub(crate) fn new(
        id: ChannelId,
        status: ChannelStatus,
        internal_rpc: InternalRPCHandle,
    ) -> Self {
        Self {
            id,
            status,
            internal_rpc,
        }
    }
}

impl fmt::Debug for ChannelCloser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChannelCloser")
            .field("id", &self.id)
            .field("status", &self.status)
            .finish()
    }
}

impl Drop for ChannelCloser {
    fn drop(&mut self) {
        if self.status.auto_close(self.id) {
            self.internal_rpc.close_channel(
                self.id,
                protocol::constants::REPLY_SUCCESS,
                "OK".to_string(),
            );
        }
    }
}
