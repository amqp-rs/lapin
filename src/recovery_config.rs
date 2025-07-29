use crate::Error;

#[derive(Default, Clone)]
pub struct RecoveryConfig {
    auto_recover_channels: bool,
    auto_recover_connection: bool,
}

impl RecoveryConfig {
    #[cfg(feature = "unstable")]
    pub fn auto_recover_channels(mut self) -> Self {
        self.auto_recover_channels = true;
        self
    }

    pub(crate) fn can_recover_channel(&self, error: &Error) -> bool {
        self.auto_recover_channels && error.is_amqp_soft_error()
    }

    pub(crate) fn can_recover_connection(&self, error: &Error) -> bool {
        self.auto_recover_connection && error.can_be_recovered()
    }

    pub(crate) fn can_recover(&self, error: &Error) -> bool {
        self.can_recover_channel(error) || self.can_recover_connection(error)
    }
}
