use crate::channels::Channels;
use executor_trait::FullExecutor;
use parking_lot::Mutex;
use reactor_trait::Reactor;
use std::{
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Heartbeat {
    channels: Channels,
    executor: Arc<dyn FullExecutor + Send + Sync>,
    reactor: Arc<dyn Reactor + Send + Sync>,
    inner: Arc<Mutex<Inner>>,
}

impl Heartbeat {
    pub(crate) fn new(
        channels: Channels,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        reactor: Arc<dyn Reactor + Send + Sync>,
    ) -> Self {
        let inner = Default::default();
        Self {
            channels,
            executor,
            reactor,
            inner,
        }
    }

    pub(crate) fn set_timeout(&self, timeout: Duration) {
        self.inner.lock().timeout = Some(timeout);
    }

    pub(crate) fn start(&self) {
        let heartbeat = self.clone();
        self.executor.spawn(Box::pin(async move {
            while let Some(dur) = heartbeat.poll_timeout() {
                heartbeat.reactor.sleep(dur).await;
            }
        }));
    }

    fn poll_timeout(&self) -> Option<Duration> {
        self.inner.lock().poll_timeout(&self.channels)
    }

    pub(crate) fn update_last_write(&self) {
        self.inner.lock().update_last_write();
    }

    pub(crate) fn cancel(&self) {
        self.inner.lock().timeout = None;
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
    fn poll_timeout(&mut self, channels: &Channels) -> Option<Duration> {
        self.timeout.map(|timeout| {
            timeout
                .checked_sub(self.last_write.elapsed())
                .map(|timeout| timeout.max(Duration::from_millis(1)))
                .unwrap_or_else(|| {
                    // Update last_write so that if we cannot write to the socket yet, we don't enqueue countless heartbeats
                    self.update_last_write();
                    channels.send_heartbeat();
                    timeout
                })
        })
    }

    fn update_last_write(&mut self) {
        self.last_write = Instant::now();
    }
}
