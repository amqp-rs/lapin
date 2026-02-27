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

pub struct Configuration {
    pub(crate) amqp_client_properties: FieldTable,
    pub(crate) amqp_locale: ShortString,
    pub(crate) auth_provider: Arc<dyn AuthProvider>,
    pub(crate) backoff: ExponentialBuilder,
    pub(crate) negotiated_config: NegotiatedConfig,
    pub(crate) auto_recover: bool,
}

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

    pub fn channel_max(&self) -> ChannelId {
        self.negotiated_config.channel_max()
    }

    pub fn frame_max(&self) -> FrameSize {
        self.negotiated_config.frame_max()
    }

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
    channel_max: ChannelId,
    frame_max: FrameSize,
    heartbeat: Heartbeat,
}

impl NegotiatedConfig {
    fn new(uri: &AMQPUri) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                frame_max: uri.query.frame_max.unwrap_or_default(),
                channel_max: uri.query.channel_max.unwrap_or_default(),
                heartbeat: uri.query.heartbeat.unwrap_or_default(),
            })),
        }
    }

    pub(crate) fn channel_max(&self) -> ChannelId {
        self.read_inner().channel_max
    }

    pub(crate) fn set_channel_max(&self, channel_max: ChannelId) {
        self.write_inner().channel_max = channel_max;
    }

    pub(crate) fn frame_max(&self) -> FrameSize {
        self.read_inner().frame_max
    }

    pub(crate) fn set_frame_max(&self, frame_max: FrameSize) {
        let frame_max = std::cmp::max(frame_max, protocol::constants::FRAME_MIN_SIZE);
        self.write_inner().frame_max = frame_max;
    }

    pub(crate) fn heartbeat(&self) -> Heartbeat {
        self.read_inner().heartbeat
    }

    pub(crate) fn set_heartbeat(&self, heartbeat: Heartbeat) {
        self.write_inner().heartbeat = heartbeat;
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
