use crate::{ConnectionStatus, ErrorKind, channels::Channels, killswitch::KillSwitch};
use async_rs::{Runtime, traits::*};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};
use tracing::error;

#[derive(Clone)]
pub struct Heartbeat<RK: RuntimeKit + Clone + Send + 'static> {
    connection_status: ConnectionStatus,
    killswitch: KillSwitch,
    runtime: Runtime<RK>,
    inner: Arc<Mutex<Inner>>,
}

impl<RK: RuntimeKit + Clone + Send + 'static> Heartbeat<RK> {
    pub(crate) fn new(connection_status: ConnectionStatus, runtime: Runtime<RK>) -> Self {
        let killswitch = Default::default();
        let inner = Default::default();
        Self {
            connection_status,
            killswitch,
            runtime,
            inner,
        }
    }

    pub(crate) fn set_timeout(&self, timeout: Duration) {
        self.lock_inner().timeout = Some(timeout);
    }

    pub(crate) fn killswitch(&self) -> KillSwitch {
        self.killswitch.clone()
    }

    pub(crate) fn start(&self, channels: Channels) {
        let heartbeat = self.clone();
        self.runtime.spawn(async move {
            while let Some(dur) = heartbeat.poll_timeout(&channels) {
                heartbeat.runtime.sleep(dur).await;
            }
        });
    }

    fn poll_timeout(&self, channels: &Channels) -> Option<Duration> {
        if !self.connection_status.connected() {
            self.cancel();
            return None;
        }

        self.lock_inner().poll_timeout(channels, &self.killswitch)
    }

    pub(crate) fn update_last_write(&self) {
        self.lock_inner().update_last_write();
    }

    pub(crate) fn update_last_read(&self) {
        self.lock_inner().update_last_read();
    }

    pub(crate) fn cancel(&self) {
        self.lock_inner().timeout = None;
    }

    pub(crate) fn reset(&self) {
        self.killswitch.reset();
        self.lock_inner().reset();
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl<RK: RuntimeKit + Clone + Send + 'static> fmt::Debug for Heartbeat<RK> {
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
            error!(
                "We haven't received anything from the server for too long, closing connection."
            );
            self.timeout = None;
            killswitch.kill();
            channels.set_connection_error(ErrorKind::MissingHeartbeatError.into());
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

    fn reset(&mut self) {
        self.update_last_read();
        self.update_last_write();
        self.timeout = None;
    }
}
