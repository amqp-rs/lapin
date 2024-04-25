use crate::{channels::Channels, killswitch::KillSwitch, ConnectionStatus, Error};
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
    connection_status: ConnectionStatus,
    channels: Channels,
    killswitch: KillSwitch,
    executor: Arc<dyn FullExecutor + Send + Sync>,
    reactor: Arc<dyn Reactor + Send + Sync>,
    inner: Arc<Mutex<Inner>>,
}

impl Heartbeat {
    pub(crate) fn new(
        connection_status: ConnectionStatus,
        channels: Channels,
        executor: Arc<dyn FullExecutor + Send + Sync>,
        reactor: Arc<dyn Reactor + Send + Sync>,
    ) -> Self {
        let killswitch = Default::default();
        let inner = Default::default();
        Self {
            connection_status,
            channels,
            killswitch,
            executor,
            reactor,
            inner,
        }
    }

    pub(crate) fn set_timeout(&self, timeout: Duration) {
        self.inner.lock().timeout = Some(timeout);
    }

    pub(crate) fn killswitch(&self) -> KillSwitch {
        self.killswitch.clone()
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
        if !self.connection_status.connected() {
            self.cancel();
            return None;
        }

        self.inner
            .lock()
            .poll_timeout(&self.channels, &self.killswitch)
    }

    pub(crate) fn update_last_write(&self) {
        self.inner.lock().update_last_write();
    }

    pub(crate) fn update_last_read(&mut self) {
        self.inner.lock().update_last_read();
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
    last_read: Instant,
    last_write: Instant,
    timeout: Option<Duration>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            last_read: Instant::now(),
            last_write: Instant::now(),
            timeout: None,
        }
    }
}

impl Inner {
    fn poll_timeout(&mut self, channels: &Channels, killswitch: &KillSwitch) -> Option<Duration> {
        let timeout = self.timeout?;

        // The value stored in timeout is half the configured heartbeat value as the spec recommends to send heartbeats at twice the configured pace.
        // The specs tells us to close the connection after twice the configured interval has passed.
        if Instant::now().duration_since(self.last_read) > 4 * timeout {
            self.timeout = None;
            killswitch.kill();
            channels.set_connection_error(Error::MissingHeartbeatError);
            return None;
        }

        timeout
            .checked_sub(self.last_write.elapsed())
            .map(|timeout| timeout.max(Duration::from_millis(1)))
            .or_else(|| {
                // Update last_write so that if we cannot write to the socket yet, we don't enqueue countless heartbeats
                self.update_last_write();
                channels.send_heartbeat();
                Some(timeout)
            })
    }

    fn update_last_write(&mut self) {
        self.last_write = Instant::now();
    }

    fn update_last_read(&mut self) {
        self.last_read = Instant::now();
    }
}
