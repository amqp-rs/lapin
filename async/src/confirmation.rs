pub use crate::wait::NotifyReady;

use std::fmt;

use crate:: {
  error::Error,
  wait::Wait,
};

pub struct Confirmation<T, I=()> {
  kind: ConfirmationKind<T, I>,
}

impl<T, I> Confirmation<T, I> {
  pub(crate) fn new(wait: Wait<T>) -> Self {
    Self { kind: ConfirmationKind::Wait(wait) }
  }

  pub(crate) fn new_error(error: Error) -> Self {
    let (wait, wait_handle) = Wait::new();
    wait_handle.error(error);
    Self::new(wait)
  }

  pub fn as_error(self) -> Result<(), Error> {
    self.try_wait().transpose().map(|_| ())
  }

  pub fn subscribe(&self, task: Box<dyn NotifyReady + Send>) {
    match &self.kind {
      ConfirmationKind::Wait(wait)   => wait.subscribe(task),
      ConfirmationKind::Map(wait, _) => wait.subscribe(task),
    }
  }

  pub fn try_wait(&self) -> Option<Result<T, Error>> {
    match &self.kind {
      ConfirmationKind::Wait(wait)   => wait.try_wait(),
      ConfirmationKind::Map(wait, f) => wait.try_wait().map(|res| res.map(f)),
    }
  }

  pub fn wait(self) -> Result<T, Error> {
    match self.kind {
      ConfirmationKind::Wait(wait)   => wait.wait(),
      ConfirmationKind::Map(wait, f) => wait.wait().map(f),
    }
  }
}

impl<T> Confirmation<T> {
  pub(crate) fn map<M>(self, f: Box<dyn Fn(T) -> M + Send + 'static>) -> Confirmation<M, T> {
    Confirmation { kind: ConfirmationKind::Map(Box::new(self), f) }
  }
}

enum ConfirmationKind<T, I> {
  Wait(Wait<T>),
  Map(Box<Confirmation<I>>, Box<dyn Fn(I) -> T + Send + 'static>)
}

impl<T> From<Result<Wait<T>, Error>> for Confirmation<T> {
  fn from(res: Result<Wait<T>, Error>) -> Self {
    match res {
      Ok(wait) => Confirmation::new(wait),
      Err(err) => Confirmation::new_error(err),
    }
  }
}

impl<T> fmt::Debug for Confirmation<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Confirmation")
  }
}
