use crate::{consumer_status::ConsumerState, internal_rpc::InternalRPCHandle};
use parking_lot::Mutex;
use std::sync::Arc;

pub(crate) struct ConsumerCanceler {
    channel_id: u16,
    consumer_tag: String,
    state: Arc<Mutex<ConsumerState>>,
    internal_rpc: InternalRPCHandle,
}

impl ConsumerCanceler {
    pub(crate) fn new(
        channel_id: u16,
        consumer_tag: String,
        state: Arc<Mutex<ConsumerState>>,
        internal_rpc: InternalRPCHandle,
    ) -> Self {
        Self {
            channel_id,
            consumer_tag,
            state,
            internal_rpc,
        }
    }
}

impl Drop for ConsumerCanceler {
    fn drop(&mut self) {
        let state = self.state.lock();
        if *state == ConsumerState::Active {
            self.internal_rpc
                .cancel_consumer(self.channel_id, self.consumer_tag.clone());
        }
    }
}
