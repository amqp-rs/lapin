use parking_lot::{Mutex, MutexGuard};
use std::{fmt, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct ConsumerStatus(Arc<Mutex<ConsumerStatusInner>>);

impl ConsumerStatus {
    pub(crate) fn state(&self) -> ConsumerState {
        self.lock().state()
    }

    pub(crate) fn try_lock(&self) -> Option<MutexGuard<'_, ConsumerStatusInner>> {
        self.0.try_lock()
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, ConsumerStatusInner> {
        self.0.lock()
    }
}

impl fmt::Debug for ConsumerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ConsumerStatus");
        if let Some(inner) = self.try_lock() {
            debug.field("state", &inner.state());
        }
        debug.finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumerState {
    Active,
    ActiveWithDelegate,
    Canceling,
    Canceled,
}

impl ConsumerState {
    pub fn is_active(self) -> bool {
        match self {
            ConsumerState::Active | ConsumerState::ActiveWithDelegate => true,
            _ => false,
        }
    }
}

impl Default for ConsumerState {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Default)]
pub(crate) struct ConsumerStatusInner(ConsumerState);

impl ConsumerStatusInner {
    pub(crate) fn state(&self) -> ConsumerState {
        self.0
    }

    pub(crate) fn set_delegate(&mut self) {
        if self.0 == ConsumerState::Active {
            self.0 = ConsumerState::ActiveWithDelegate;
        }
    }

    pub(crate) fn start_cancel(&mut self) {
        self.0 = ConsumerState::Canceling;
    }

    pub(crate) fn cancel(&mut self) {
        self.0 = ConsumerState::Canceled;
    }
}
