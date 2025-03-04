#[derive(Default, Clone)]
pub struct RecoveryConfig {
    pub(crate) auto_recover_channels: bool,
}

impl RecoveryConfig {
    #[cfg(feature = "unstable")]
    pub fn auto_recover_channels(mut self) -> Self {
        self.auto_recover_channels = true;
        self
    }
}
