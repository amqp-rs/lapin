use std::{
    fmt,
    ops::{AddAssign, Sub},
};

pub(crate) struct IdSequence<T> {
    allow_zero: bool,
    zero: T,
    one: T,
    max: Option<T>,
    id: T,
}

impl<
        T: Default + Copy + AddAssign<T> + Sub<Output = T> + PartialEq<T> + PartialOrd<T> + From<u8>,
    > IdSequence<T>
{
    pub(crate) fn new(allow_zero: bool) -> Self {
        Self {
            allow_zero,
            zero: 0.into(),
            one: 1.into(),
            max: None,
            id: T::default(),
        }
    }

    pub(crate) fn current(&self) -> Option<T> {
        // self.id is actually the next (so that first call to next returns 0
        // if we're 0 (or 1 if 0 is not allowed), either we haven't started yet, or last number we yielded (current one) is
        // the max.
        if self.id <= self.first() {
            self.max
        } else {
            Some(self.id - self.one)
        }
    }

    fn first(&self) -> T {
        if self.allow_zero {
            self.zero
        } else {
            self.one
        }
    }

    pub(crate) fn set_max(&mut self, max: T) {
        self.max = if max == self.zero { None } else { Some(max) };
    }

    pub(crate) fn next(&mut self) -> T {
        if !self.allow_zero && self.id == self.zero {
            self.id += self.one;
        }
        let id = self.id;
        if self.max == Some(id) {
            self.id = self.zero;
        } else {
            self.id += self.one;
        }
        id
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_low_max() {
        let mut sequence = IdSequence::<u8>::new(false);
        sequence.set_max(10);
        for i in 1..=10 {
            assert_eq!(sequence.next(), i)
        }
        // We should cycle back to first correct value
        assert_eq!(sequence.next(), 1)
    }

    #[test]
    fn test_highest_max() {
        let mut sequence = IdSequence::<u8>::new(false);
        sequence.set_max(u8::MAX);
        for i in 1..=u8::MAX {
            assert_eq!(sequence.next(), i)
        }
        // We should cycle back to first correct value
        assert_eq!(sequence.next(), 1)
    }
}
