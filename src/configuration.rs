use crate::{
    ConnectionProperties, Error,
    auth::{AuthProvider, DefaultAuthProvider},
    protocol,
    types::{ChannelId, FieldTable, FrameSize, Heartbeat, ShortString},
    uri::AMQPUri,
};
use backon::ExponentialBuilder;
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// The negotiated connection parameters returned by the AMQP handshake.
///
/// Obtained from [`Connection::configuration`].
///
/// [`Connection::configuration`]: crate::Connection::configuration
pub struct Configuration {
    pub(crate) amqp_client_properties: FieldTable,
    pub(crate) amqp_locale: ShortString,
    pub(crate) auth_provider: Arc<dyn AuthProvider>,
    pub(crate) backoff: ExponentialBuilder,
    pub(crate) negotiated_config: NegotiatedConfig,
    pub(crate) auto_recover: bool,
}

// A zero URI frame_max means no requested cap. Keep that case bounded so a
// broker's unlimited offer cannot make an untrusted frame allocate gigabytes.
pub(crate) const DEFAULT_FRAME_MAX: FrameSize = 16 * 1024 * 1024;

impl Configuration {
    pub(crate) fn new(uri: &AMQPUri, options: ConnectionProperties) -> Self {
        let ConnectionProperties {
            locale,
            client_properties,
            auth_provider,
            backoff,
            auto_recover,
            ..
        } = options;
        Self {
            amqp_client_properties: client_properties,
            amqp_locale: locale,
            auth_provider: auth_provider.unwrap_or_else(|| Arc::new(DefaultAuthProvider::new(uri))),
            backoff,
            negotiated_config: NegotiatedConfig::new(uri),
            auto_recover,
        }
    }

    /// Maximum number of channels allowed on this connection (negotiated with the server).
    #[must_use]
    pub fn channel_max(&self) -> ChannelId {
        self.negotiated_config.channel_max()
    }

    /// Maximum AMQP frame size in bytes (negotiated with the server).
    #[must_use]
    pub fn frame_max(&self) -> FrameSize {
        self.negotiated_config.frame_max()
    }

    /// Heartbeat interval in seconds (negotiated with the server). `0` means disabled.
    #[must_use]
    pub fn heartbeat(&self) -> Heartbeat {
        self.negotiated_config.heartbeat()
    }

    pub(crate) fn recovery_config(&self) -> RecoveryConfig {
        RecoveryConfig(self.auto_recover)
    }
}

impl Clone for Configuration {
    fn clone(&self) -> Self {
        Self {
            amqp_client_properties: self.amqp_client_properties.clone(),
            amqp_locale: self.amqp_locale.clone(),
            auth_provider: self.auth_provider.clone(),
            backoff: self.backoff,
            negotiated_config: self.negotiated_config.clone(),
            auto_recover: self.auto_recover,
        }
    }
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.negotiated_config, f)
    }
}

#[derive(Clone)]
pub(crate) struct NegotiatedConfig {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Default, Clone, Copy)]
pub(crate) struct RecoveryConfig(pub(crate) bool);

struct Inner {
    requested_channel_max: ChannelId,
    requested_frame_max: FrameSize,
    requested_heartbeat: Heartbeat,
    channel_max: ChannelId,
    frame_max: FrameSize,
    heartbeat: Heartbeat,
}

impl NegotiatedConfig {
    fn new(uri: &AMQPUri) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                requested_channel_max: uri.query.channel_max.unwrap_or_default(),
                requested_frame_max: uri.query.frame_max.unwrap_or_default(),
                requested_heartbeat: uri.query.heartbeat.unwrap_or_default(),
                frame_max: uri.query.frame_max.unwrap_or_default(),
                channel_max: uri.query.channel_max.unwrap_or_default(),
                heartbeat: uri.query.heartbeat.unwrap_or_default(),
            })),
        }
    }

    pub(crate) fn channel_max(&self) -> ChannelId {
        self.read_inner().channel_max
    }

    #[cfg(test)]
    pub(crate) fn set_channel_max(&self, channel_max: ChannelId) {
        self.write_inner().channel_max = channel_max;
    }

    pub(crate) fn frame_max(&self) -> FrameSize {
        self.read_inner().frame_max
    }

    pub(crate) fn pre_tune_frame_max(&self) -> FrameSize {
        self.read_inner().requested_frame_max.max(DEFAULT_FRAME_MAX)
    }

    pub(crate) fn heartbeat(&self) -> Heartbeat {
        self.read_inner().heartbeat
    }

    pub(crate) fn negotiate(
        &self,
        channel_max: ChannelId,
        frame_max: FrameSize,
        heartbeat: Heartbeat,
    ) {
        let mut inner = self.write_inner();
        inner.channel_max = match (inner.requested_channel_max, channel_max) {
            (0, 0) => ChannelId::MAX,
            (0, server) => server,
            (client, 0) => client,
            (client, server) => client.min(server),
        };
        inner.frame_max = match (inner.requested_frame_max, frame_max) {
            (0, 0) => DEFAULT_FRAME_MAX,
            (0, server) => server.min(DEFAULT_FRAME_MAX),
            (client, 0) => client,
            (client, server) => client.min(server),
        }
        .max(protocol::constants::FRAME_MIN_SIZE);
        inner.heartbeat = match (inner.requested_heartbeat, heartbeat) {
            (0, server) => server,
            (client, 0) => client,
            (client, server) => client.min(server),
        };
    }

    fn read_inner(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write_inner(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for NegotiatedConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self.read_inner();
        f.debug_struct("Configuration")
            .field("channel_max", &inner.channel_max)
            .field("frame_max", &inner.frame_max)
            .field("heartbeat", &inner.heartbeat)
            .finish()
    }
}

impl RecoveryConfig {
    pub(crate) fn can_recover(&self, error: &Error) -> bool {
        self.0 && error.can_be_recovered()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_negotiates_against_uri_limits_not_previous_broker() {
        let default = NegotiatedConfig::new(&AMQPUri::default());
        default.negotiate(1, 4096, 5);
        default.negotiate(100, 65536, 20);
        assert_eq!(default.channel_max(), 100);
        assert_eq!(default.frame_max(), 65536);
        assert_eq!(default.heartbeat(), 20);

        let mut uri = AMQPUri::default();
        uri.query.channel_max = Some(10);
        uri.query.frame_max = Some(8192);
        uri.query.heartbeat = Some(10);
        let capped = NegotiatedConfig::new(&uri);
        capped.negotiate(1, 4096, 5);
        capped.negotiate(100, 65536, 20);
        assert_eq!(capped.channel_max(), 10);
        assert_eq!(capped.frame_max(), 8192);
        assert_eq!(capped.heartbeat(), 10);
    }
}
