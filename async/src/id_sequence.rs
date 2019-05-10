use parking_lot::Mutex;

use std::{
  ops::AddAssign,
  sync::Arc,
};

#[derive(Clone, Debug)]
pub struct IdSequence<T> {
  inner: Arc<Mutex<Inner<T>>>,
}

impl<T: Default + Copy + AddAssign<T> + PartialEq<T> + PartialOrd<T> + From<u8>> IdSequence<T> {
  pub fn new(allow_zero: bool) -> Self {
    Self {
      inner: Arc::new(Mutex::new(Inner::new(allow_zero)))
    }
  }

  pub fn next(&self) -> T {
    self.inner.lock().next()
  }

  pub fn set_max(&self, max: T) {
    self.inner.lock().max = Some(max);
  }
}

#[derive(Debug)]
pub struct Inner<T> {
  allow_zero: bool,
  zero:       T,
  one:        T,
  max:        Option<T>,
  id:         T,
}

impl<T: Default + Copy + AddAssign<T> + PartialEq<T> + PartialOrd<T> + From<u8>> Inner<T> {
  fn new(allow_zero: bool) -> Self {
    Self {
      allow_zero,
      zero: 0.into(),
      one:  1.into(),
      max:  None,
      id:   T::default(),
    }
  }

  // FIXME: use Step trait once stable (https://github.com/rust-lang/rust/issues/42168)
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
