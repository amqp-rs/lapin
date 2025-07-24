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

    pub(crate) fn reset(&self) {
        self.0.store(false, Ordering::SeqCst)
    }
}
