use crate::{
    protocol,
    types::{ChannelId, FrameSize, Heartbeat},
};
use parking_lot::RwLock;
use std::{fmt, sync::Arc};

#[derive(Clone, Default)]
pub struct Configuration {
    inner: Arc<RwLock<Inner>>,
}

impl Configuration {
    pub fn channel_max(&self) -> ChannelId {
        self.inner.read().channel_max
    }

    pub(crate) fn set_channel_max(&self, channel_max: ChannelId) {
        self.inner.write().channel_max = channel_max;
    }

    pub fn frame_max(&self) -> FrameSize {
        self.inner.read().frame_max
    }

    pub(crate) fn set_frame_max(&self, frame_max: FrameSize) {
        let frame_max = std::cmp::max(frame_max, protocol::constants::FRAME_MIN_SIZE);
        self.inner.write().frame_max = frame_max;
    }

    pub fn heartbeat(&self) -> Heartbeat {
        self.inner.read().heartbeat
    }

    pub(crate) fn set_heartbeat(&self, heartbeat: Heartbeat) {
        self.inner.write().heartbeat = heartbeat;
    }
}

#[derive(Default)]
struct Inner {
    channel_max: ChannelId,
    frame_max: FrameSize,
    heartbeat: Heartbeat,
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.inner.read();
        f.debug_struct("Configuration")
            .field("channel_max", &inner.channel_max)
            .field("frame_max", &inner.frame_max)
            .field("heartbeat", &inner.heartbeat)
            .finish()
    }
}
