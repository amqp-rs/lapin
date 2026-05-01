use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default, Debug, Clone)]
pub(crate) struct KillSwitch(Arc<AtomicBool>);

impl KillSwitch {
    pub(crate) fn kill(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }

    pub(crate) fn killed(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    // Only safe to call from the IO loop thread during channel recovery, after all
    // in-flight operations for this channel have been cancelled. No concurrent
    // kill() calls may be in progress at that point, so the unconditional store is safe.
    pub(crate) fn reset(&self) {
        self.0.store(false, Ordering::SeqCst)
    }
}
