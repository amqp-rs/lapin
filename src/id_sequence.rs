use parking_lot::Mutex;
use std::{fmt, ops::AddAssign, sync::Arc};

#[derive(Clone)]
pub(crate) struct IdSequence<T>(Arc<Mutex<Inner<T>>>);

impl<T: Default + Copy + AddAssign<T> + PartialEq<T> + PartialOrd<T> + From<u8>> IdSequence<T> {
    pub(crate) fn new(allow_zero: bool) -> Self {
        Self(Arc::new(Mutex::new(Inner::new(allow_zero))))
    }

    pub(crate) fn next(&self) -> T {
        self.0.lock().next()
    }

    pub(crate) fn set_max(&self, max: T) {
        self.0.lock().set_max(max)
    }
}

impl<T: fmt::Debug> fmt::Debug for IdSequence<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("IdSequence");
        if let Some(inner) = self.0.try_lock() {
            debug
                .field("allow_zero", &inner.allow_zero)
                .field("max", &inner.max)
                .field("id", &inner.id);
        }
        debug.finish()
    }
}

#[derive(Debug)]
struct Inner<T> {
    allow_zero: bool,
    zero: T,
    one: T,
    max: Option<T>,
    id: T,
}

impl<T: Default + Copy + AddAssign<T> + PartialEq<T> + PartialOrd<T> + From<u8>> Inner<T> {
    fn new(allow_zero: bool) -> Self {
        Self {
            allow_zero,
            zero: 0.into(),
            one: 1.into(),
            max: None,
            id: T::default(),
        }
    }

    fn set_max(&mut self, max: T) {
        self.max = if max == self.zero { None } else { Some(max) };
    }

    fn next(&mut self) -> T {
        if !self.allow_zero && self.id == self.zero {
            self.id += self.one;
        }
        if self.check_max() {
            let id = self.id;
            self.id += self.one;
            id
        } else {
            self.id = self.zero;
            self.next()
        }
    }

    fn check_max(&self) -> bool {
        if let Some(max) = self.max {
            self.id < max
        } else {
            true
        }
    }
}
