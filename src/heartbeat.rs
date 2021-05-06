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
        let mut inner = self.inner.lock();
        if let Some(timeout) = inner.timeout.as_ref() {
            let elapsed = inner.last_write.elapsed();
            let dur_elapsed = timeout
                .checked_sub(elapsed)
                .map(|timeout| timeout.max(Duration::from_millis(1)));
            if dur_elapsed.is_none() {
                // Update last_write so that if we cannot write to the socket yet, we don't enqueue countless heartbeats
                inner.update_last_write();
                self.channels.send_heartbeat();
            }
            dur_elapsed
        } else {
            None
        }
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
    fn update_last_write(&mut self) {
        self.last_write = Instant::now();
    }
}
