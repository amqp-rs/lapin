use parking_lot::Mutex;

use std::{
  ops::AddAssign,
  sync::Arc,
};

#[derive(Clone, Debug, Default)]
pub struct IdSequence<T> {
  allow_zero: bool,
  zero:       T,
  one:        T,
  id:         Arc<Mutex<T>>,
}

// FIXME: use Step trait once stable (https://github.com/rust-lang/rust/issues/42168)
impl<T: Default + Copy + AddAssign<T> + PartialEq<T> + PartialOrd<T> + From<u8>> IdSequence<T> {
  pub fn new(allow_zero: bool) -> Self {
    Self {
      allow_zero,
      zero: 0.into(),
      one: 1.into(),
      id: Default::default(),
    }
  }

  pub fn next(&self) -> T {
    let mut id = self.id.lock();
    if !self.allow_zero && *id == self.zero {
      *id += self.one;
    }
    let next_id = *id;
    *id += self.one;
    next_id
  }

  pub fn next_with_max(&self, max: T) -> T {
    let id = self.next();
    if id < max {
      id
    } else {
      *self.id.lock() = self.zero;
      self.next()
    }
  }
}
