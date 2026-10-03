use crate::{
    ConnectionStatus, Error, ErrorKind, auth::AuthProvider, internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
};
use async_rs::{Runtime, traits::*};
use std::{fmt, sync::Arc, time::Duration};
use tracing::error;

pub(crate) struct SecretUpdate<RK: RuntimeKit + Clone + Send + 'static> {
    connection_status: ConnectionStatus,
    runtime: Runtime<RK>,
    provider: Arc<dyn AuthProvider>,
    killswitch: KillSwitch,
}

impl<RK: RuntimeKit + Clone + Send + 'static> Clone for SecretUpdate<RK> {
    fn clone(&self) -> Self {
        Self {
            connection_status: self.connection_status.clone(),
            runtime: self.runtime.clone(),
            provider: self.provider.clone(),
            killswitch: self.killswitch.clone(),
        }
    }
}

impl<RK: RuntimeKit + Clone + Send + 'static> SecretUpdate<RK> {
    pub(crate) fn new(
        connection_status: ConnectionStatus,
        runtime: Runtime<RK>,
        provider: Arc<dyn AuthProvider>,
    ) -> Self {
        Self {
            connection_status,
            runtime,
            provider,
            killswitch: KillSwitch::default(),
        }
    }

    pub(crate) fn start(&self, internal_rpc: InternalRPCHandle) {
        let secret_update = self.clone();
        let killswitch = self.killswitch.clone();
        self.runtime.spawn(async move {
            while let Some(dur) = secret_update.poll_timeout(&killswitch) {
                secret_update.runtime.sleep(dur).await;
                if killswitch.killed() || !secret_update.connection_status.connected() {
                    break;
                }
                match secret_update
                    .provider
                    .refresh()
                    .map_err(|e| Error::from(ErrorKind::AuthProviderError(e)))
                {
                    Err(err) => error!(%err, "Failed refreshing secret"),
                    Ok(token) => {
                        if let Err(err) = internal_rpc
                            .update_secret(token, "Automatic periodical refresh".into())
                            .await
                        {
                            error!(%err, "Failed refreshing secret");
                        }
                    }
                }
            }
        });
    }

    pub(crate) fn cancel(&self) {
        self.killswitch.kill();
    }

    pub(crate) fn reset(&mut self) {
        self.cancel();
        self.killswitch = KillSwitch::default();
    }

    fn poll_timeout(&self, killswitch: &KillSwitch) -> Option<Duration> {
        if killswitch.killed() || !self.connection_status.connected() {
            return None;
        }

        self.provider.valid_for().map(refresh_delay)
    }
}

fn refresh_delay(valid_for: Duration) -> Duration {
    (valid_for / 2).max(Duration::from_millis(1))
}

impl<RK: RuntimeKit + Clone + Send + 'static> fmt::Debug for SecretUpdate<RK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretUpdate").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::refresh_delay;
    use std::time::Duration;

    #[test]
    fn refresh_is_scheduled_before_expiry() {
        assert_eq!(
            refresh_delay(Duration::from_secs(30)),
            Duration::from_secs(15)
        );
        assert_eq!(refresh_delay(Duration::ZERO), Duration::from_millis(1));
    }
}
