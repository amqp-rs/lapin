#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsumerState {
    Active,
    Canceled,
}

impl Default for ConsumerState {
    fn default() -> Self {
        Self::Active
    }
}
