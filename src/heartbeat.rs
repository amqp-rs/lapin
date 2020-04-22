use crate::{channels::Channels, Result};
use parking_lot::Mutex;
use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Heartbeat {
    channels: Channels,
    inner: Arc<Mutex<Inner>>,
}

impl Heartbeat {
    pub(crate) fn new(channels: Channels) -> Self {
        let inner = Default::default();
        Self { channels, inner }
    }

    pub(crate) fn set_timeout(&self, timeout: Duration) {
        self.inner.lock().timeout = Some(timeout);
    }

    pub fn get_heartbeat(&self) -> Option<Duration> {
        self.inner.lock().timeout
    }

    pub fn poll_timeout(&self) -> Result<Option<Duration>> {
        self.inner.lock().poll_timeout(&self.channels)
    }

    pub fn send(&self) -> Result<()> {
        self.channels.send_heartbeat()
    }

    pub(crate) fn update_last_write(&self) {
        self.inner.lock().update_last_write();
    }
}

impl fmt::Debug for Heartbeat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Heartbeat").finish()
    }
}

struct Inner {
    last_write: Instant,
    timeout: Option<Duration>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            last_write: Instant::now(),
            timeout: None,
        }
    }
}

impl Inner {
    fn poll_timeout(&mut self, channels: &Channels) -> Result<Option<Duration>> {
        self.timeout
            .map(|timeout| {
                timeout
                    .checked_sub(self.last_write.elapsed())
                    .map(Ok)
                    .unwrap_or_else(|| {
                        // Update last_write so that if we cannot write to the socket yet, we don't enqueue countless heartbeats
                        self.update_last_write();
                        channels.send_heartbeat()?;
                        Ok(timeout)
                    })
            })
            .transpose()
    }

    fn update_last_write(&mut self) {
        self.last_write = Instant::now();
    }
}
