use crate::{
    ConnectionStatus, Error, ErrorKind, auth::AuthProvider, internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
};
use async_rs::{Runtime, traits::*};
use std::{fmt, sync::Arc, time::Duration};
use tracing::error;

pub struct SecretUpdate<RK: RuntimeKit + Clone + Send + 'static> {
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
        self.runtime.spawn(async move {
            while let Some(dur) = secret_update.poll_timeout() {
                secret_update.runtime.sleep(dur).await;
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

    pub(crate) fn reset(&self) {
        self.killswitch.reset();
    }

    fn poll_timeout(&self) -> Option<Duration> {
        if !self.connection_status.connected() || self.killswitch.killed() {
            return None;
        }

        self.provider.valid_for()
    }
}

impl<RK: RuntimeKit + Clone + Send + 'static> fmt::Debug for SecretUpdate<RK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretUpdate").finish()
    }
}
