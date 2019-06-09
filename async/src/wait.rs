use parking_lot::Mutex;

use std::{
  fmt,
  sync::{
    Arc,
    mpsc::{SyncSender, Receiver, sync_channel},
  },
};

pub struct Wait<T> {
  recv: Receiver<T>,
  task: Arc<Mutex<Option<Box<dyn NotifyReady + Send>>>>,
}

#[derive(Clone)]
pub struct WaitHandle<T> {
  send: SyncSender<T>,
  task: Arc<Mutex<Option<Box<dyn NotifyReady + Send>>>>,
}

pub trait NotifyReady {
  fn notify(&self);
}

impl<T> Wait<T> {
  pub(crate) fn new() -> (Self, WaitHandle<T>) {
    let (send, recv) = sync_channel(1);
    let task = Arc::new(Mutex::new(None));
    (Self { recv, task: task.clone() }, WaitHandle { send, task })
  }

  pub(crate) fn try_wait(&self) -> Option<T> {
    self.recv.try_recv().ok()
  }

  pub(crate) fn wait(&self) -> T {
    self.recv.recv().unwrap()
  }

  pub(crate) fn subscribe(&self, task: Box<dyn NotifyReady + Send>) {
    *self.task.lock() = Some(task);
  }
}

impl<T> WaitHandle<T> {
  pub(crate) fn finish(&self, val: T) {
    let _ = self.send.send(val);
    if let Some(task) = self.task.lock().take() {
      task.notify();
    }
  }
}

impl<T> fmt::Debug for Wait<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Wait")
  }
}

impl<T> fmt::Debug for WaitHandle<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "WaitHandle")
  }
}
