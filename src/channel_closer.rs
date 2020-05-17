use crate::{channel_status::ChannelStatus, internal_rpc::InternalRPCHandle, protocol};

pub(crate) struct ChannelCloser {
    id: u16,
    status: ChannelStatus,
    internal_rpc: InternalRPCHandle,
}

impl ChannelCloser {
    pub(crate) fn new(id: u16, status: ChannelStatus, internal_rpc: InternalRPCHandle) -> Self {
        Self {
            id,
            status,
            internal_rpc,
        }
    }
}

impl Drop for ChannelCloser {
    fn drop(&mut self) {
        if self.status.auto_close(self.id) {
            self.internal_rpc.close_channel(
                self.id,
                protocol::constants::REPLY_SUCCESS as u16,
                "OK".to_string(),
            );
        }
    }
}
