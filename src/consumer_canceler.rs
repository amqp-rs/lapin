use crate::{
    consumer_status::{ConsumerState, ConsumerStatus},
    internal_rpc::InternalRPCHandle,
    types::ChannelId,
};

pub(crate) struct ConsumerCanceler {
    channel_id: ChannelId,
    consumer_tag: String,
    status: ConsumerStatus,
    internal_rpc: InternalRPCHandle,
}

impl ConsumerCanceler {
    pub(crate) fn new(
        channel_id: ChannelId,
        consumer_tag: String,
        status: ConsumerStatus,
        internal_rpc: InternalRPCHandle,
    ) -> Self {
        Self {
            channel_id,
            consumer_tag,
            status,
            internal_rpc,
        }
    }
}

impl Drop for ConsumerCanceler {
    fn drop(&mut self) {
        if self.status.state() == ConsumerState::Active {
            self.internal_rpc.cancel_consumer(
                self.channel_id,
                self.consumer_tag.clone(),
                self.status.clone(),
            );
        }
    }
}
