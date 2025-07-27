#[derive(Default, Clone)]
pub struct RecoveryConfig {
    pub(crate) auto_recover_channels: bool,
    pub(crate) auto_recover_connection: bool,
}

impl RecoveryConfig {
    #[cfg(feature = "unstable")]
    pub fn auto_recover_channels(mut self) -> Self {
        self.auto_recover_channels = true;
        self
    }

    #[cfg(feature = "unstable")]
    pub fn auto_recover_connection(mut self) -> Self {
        self.auto_recover_connection = true;
        self.auto_recover_channels()
    }
}
