use crate:: {
  error::Error,
  wait::{NotifyReady, Wait},
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
    Self { kind: ConfirmationKind::Error(error) }
  }

  pub(crate) fn as_error(self) -> Result<(), Error> {
    self.into_result().map(|_| ())
  }

  pub fn subscribe(&self, task: Box<dyn NotifyReady + Send>) {
    if let ConfirmationKind::Wait(ref wait) = self.kind {
      wait.subscribe(task);
    }
  }

  pub fn into_result(self) -> Result<Self, Error> {
    if let ConfirmationKind::Error(err) = self.kind {
      Err(err)
    } else {
      Ok(self)
    }
  }

  pub fn try_wait(&self) -> Option<T> {
    match &self.kind {
      ConfirmationKind::Wait(wait) => wait.try_wait(),
      ConfirmationKind::Error(_)   => None,
    }
  }

  pub fn wait(self) -> Result<T, Error> {
    Ok(match self.kind {
      ConfirmationKind::Wait(wait)   => wait.wait(),
      ConfirmationKind::Error(error) => return Err(error),
    })
  }
}

#[derive(Debug)]
enum ConfirmationKind<T> {
  Wait(Wait<T>),
  Error(Error),
}

impl<T> From<Result<Wait<T>, Error>> for Confirmation<T> {
  fn from(res: Result<Wait<T>, Error>) -> Self {
    Self {
      kind: match res {
        Ok(wait) => ConfirmationKind::Wait(wait),
        Err(err) => ConfirmationKind::Error(err),
      }
    }
  }
}
