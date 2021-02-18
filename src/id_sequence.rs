use std::{fmt, ops::AddAssign};

pub(crate) struct IdSequence<T> {
    allow_zero: bool,
    zero: T,
    one: T,
    max: Option<T>,
    id: T,
}

impl<T: Default + Copy + AddAssign<T> + PartialEq<T> + PartialOrd<T> + From<u8>> IdSequence<T> {
    pub(crate) fn new(allow_zero: bool) -> Self {
        Self {
            allow_zero,
            zero: 0.into(),
            one: 1.into(),
            max: None,
            id: T::default(),
        }
    }

    pub(crate) fn current(&self) -> T {
        self.id
    }

    pub(crate) fn set_max(&mut self, max: T) {
        self.max = if max == self.zero { None } else { Some(max) };
    }

    pub(crate) fn next(&mut self) -> T {
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
            self.id <= max
        } else {
            true
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for IdSequence<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdSequence")
            .field("allow_zero", &self.allow_zero)
            .field("max", &self.max)
            .field("id", &self.id)
            .finish()
    }
}
