use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Default, Clone)]
pub(crate) struct KillSwitch(Arc<AtomicBool>);

impl KillSwitch {
    pub(crate) fn kill(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub(crate) fn killed(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
