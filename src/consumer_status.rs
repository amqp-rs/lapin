use parking_lot::{Mutex, MutexGuard};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(crate) struct ConsumerStatus(Arc<Mutex<ConsumerStatusInner>>);

impl ConsumerStatus {
    pub(crate) fn try_lock(&self) -> Option<MutexGuard<'_, ConsumerStatusInner>> {
        self.0.try_lock()
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, ConsumerStatusInner> {
        self.0.lock()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumerState {
    Active,
    Canceled,
}

impl Default for ConsumerState {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Default)]
pub(crate) struct ConsumerStatusInner {
    state: ConsumerState,
}

impl ConsumerStatusInner {
    pub(crate) fn state(&self) -> ConsumerState {
        self.state
    }

    pub(crate) fn cancel(&mut self) {
        self.state = ConsumerState::Canceled;
    }
}
