use crate::{
    protocol,
    types::{ChannelId, FrameSize, Heartbeat},
    uri::AMQPUri,
};
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

#[derive(Clone, Default)]
pub struct Configuration {
    inner: Arc<RwLock<Inner>>,
}

impl Configuration {
    pub(crate) fn new(uri: &AMQPUri) -> Self {
        let conf = Self::default();
        conf.init(uri);
        conf
    }

    pub fn channel_max(&self) -> ChannelId {
        self.read_inner().channel_max
    }

    pub(crate) fn set_channel_max(&self, channel_max: ChannelId) {
        self.write_inner().channel_max = channel_max;
    }

    pub fn frame_max(&self) -> FrameSize {
        self.read_inner().frame_max
    }

    pub(crate) fn set_frame_max(&self, frame_max: FrameSize) {
        let frame_max = std::cmp::max(frame_max, protocol::constants::FRAME_MIN_SIZE);
        self.write_inner().frame_max = frame_max;
    }

    pub fn heartbeat(&self) -> Heartbeat {
        self.read_inner().heartbeat
    }

    pub(crate) fn set_heartbeat(&self, heartbeat: Heartbeat) {
        self.write_inner().heartbeat = heartbeat;
    }

    fn init(&self, uri: &AMQPUri) {
        if let Some(frame_max) = uri.query.frame_max {
            self.set_frame_max(frame_max);
        }
        if let Some(channel_max) = uri.query.channel_max {
            self.set_channel_max(channel_max);
        }
        if let Some(heartbeat) = uri.query.heartbeat {
            self.set_heartbeat(heartbeat);
        }
    }

    fn read_inner(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write_inner(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
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
        let inner = self.read_inner();
        f.debug_struct("Configuration")
            .field("channel_max", &inner.channel_max)
            .field("frame_max", &inner.frame_max)
            .field("heartbeat", &inner.heartbeat)
            .finish()
    }
}
