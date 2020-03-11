use parking_lot::RwLock;

use std::sync::Arc;

#[derive(Clone, Debug, Default)]
#[deprecated(note = "use lapin instead")]
pub struct Configuration {
  inner: Arc<RwLock<Inner>>,
}

impl Configuration {
  #[deprecated(note = "use lapin instead")]
  pub fn channel_max(&self) -> u16 {
    self.inner.read().channel_max
  }

  pub(crate) fn set_channel_max(&self, channel_max: u16) {
    self.inner.write().channel_max = channel_max;
  }

  #[deprecated(note = "use lapin instead")]
  pub fn frame_max(&self) -> u32 {
    self.inner.read().frame_max
  }

  pub(crate) fn set_frame_max(&self, frame_max: u32) {
    self.inner.write().frame_max = frame_max;
  }

  #[deprecated(note = "use lapin instead")]
  pub fn heartbeat(&self) -> u16 {
    self.inner.read().heartbeat
  }

  pub(crate) fn set_heartbeat(&self, heartbeat: u16) {
    self.inner.write().heartbeat = heartbeat;
  }
}

#[derive(Debug, Default)]
struct Inner {
  channel_max: u16,
  frame_max:   u32,
  heartbeat:   u16,
}
