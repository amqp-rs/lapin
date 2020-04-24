use crate::protocol;
use parking_lot::RwLock;
use std::{fmt, sync::Arc};

#[derive(Clone, Default)]
pub struct Configuration {
    inner: Arc<RwLock<Inner>>,
}

impl Configuration {
    pub fn channel_max(&self) -> u16 {
        self.inner.read().channel_max
    }

    pub(crate) fn set_channel_max(&self, channel_max: u16) {
        self.inner.write().channel_max = channel_max;
    }

    pub fn frame_max(&self) -> u32 {
        self.inner.read().frame_max
    }

    pub(crate) fn set_frame_max(&self, frame_max: u32) {
        let frame_max = std::cmp::max(frame_max, protocol::constants::FRAME_MIN_SIZE as u32);
        self.inner.write().frame_max = frame_max;
    }

    pub fn heartbeat(&self) -> u16 {
        self.inner.read().heartbeat
    }

    pub(crate) fn set_heartbeat(&self, heartbeat: u16) {
        self.inner.write().heartbeat = heartbeat;
    }
}

#[derive(Default)]
struct Inner {
    channel_max: u16,
    frame_max: u32,
    heartbeat: u16,
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
