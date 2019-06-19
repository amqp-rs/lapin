pub use crate::wait::NotifyReady;

use crate:: {
  error::Error,
  wait::Wait,
};

#[derive(Debug)]
pub struct Confirmation<T> {
  kind: ConfirmationKind<T>,
}

impl<T> Confirmation<T> {
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
    if let ConfirmationKind::Wait(ref wait) = self.kind {
      wait.subscribe(task);
    }
  }

  pub fn try_wait(&self) -> Option<Result<T, Error>> {
    match &self.kind {
      ConfirmationKind::Wait(wait) => wait.try_wait(),
    }
  }

  pub fn wait(self) -> Result<T, Error> {
    match self.kind {
      ConfirmationKind::Wait(wait)   => wait.wait(),
    }
  }
}

#[derive(Debug)]
enum ConfirmationKind<T> {
  Wait(Wait<T>),
}

impl<T> From<Result<Wait<T>, Error>> for Confirmation<T> {
  fn from(res: Result<Wait<T>, Error>) -> Self {
    match res {
      Ok(wait) => Confirmation::new(wait),
      Err(err) => Confirmation::new_error(err),
    }
  }
}
