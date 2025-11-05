use crate::consumer::ConsumerDelegate;

use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

#[derive(Clone, Default)]
pub(crate) struct ConsumerStatus(Arc<RwLock<ConsumerStatusInner>>);

impl ConsumerStatus {
    pub(crate) fn state(&self) -> ConsumerState {
        self.read().state()
    }

    pub(crate) fn delegate(&self) -> Option<Arc<Box<dyn ConsumerDelegate>>> {
        self.read().delegate()
    }

    pub(crate) fn try_read(&self) -> Option<RwLockReadGuard<'_, ConsumerStatusInner>> {
        self.0.try_read().ok()
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<'_, ConsumerStatusInner> {
        self.0.read().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, ConsumerStatusInner> {
        self.0.write().unwrap_or_else(|e| e.into_inner())
    }
}

impl fmt::Debug for ConsumerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("ConsumerStatus");
        if let Some(inner) = self.try_read() {
            debug.field("state", &inner.state());
        }
        debug.finish()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConsumerState {
    #[default]
    Active,
    ActiveWithDelegate,
    Canceling,
    Canceled,
}

impl ConsumerState {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            ConsumerState::Active | ConsumerState::ActiveWithDelegate
        )
    }
}

#[derive(Default)]
pub(crate) struct ConsumerStatusInner {
    state: ConsumerState,
    delegate: Option<Arc<Box<dyn ConsumerDelegate>>>,
}

impl ConsumerStatusInner {
    pub(crate) fn state(&self) -> ConsumerState {
        self.state
    }

    pub(crate) fn delegate(&self) -> Option<Arc<Box<dyn ConsumerDelegate>>> {
        self.delegate.clone()
    }

    pub(crate) fn set_delegate(&mut self, delegate: Option<Arc<Box<dyn ConsumerDelegate>>>) {
        if self.state.is_active() {
            self.state = ConsumerState::ActiveWithDelegate;
            self.delegate = delegate;
        }
    }

    pub(crate) fn start_cancel(&mut self) {
        self.state = ConsumerState::Canceling;
        self.delegate = None;
    }

    pub(crate) fn cancel(&mut self) {
        self.state = ConsumerState::Canceled;
        self.delegate = None;
    }
}
