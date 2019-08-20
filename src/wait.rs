use parking_lot::Mutex;

use std::{
    fmt,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc,
    },
};

use crate::error::Error;

pub struct Wait<T> {
    recv: Receiver<Result<T, Error>>,
    send: SyncSender<Result<T, Error>>,
    task: Arc<Mutex<Option<Box<dyn NotifyReady + Send>>>>,
}

#[derive(Clone)]
pub struct WaitHandle<T> {
    send: SyncSender<Result<T, Error>>,
    task: Arc<Mutex<Option<Box<dyn NotifyReady + Send>>>>,
}

pub trait NotifyReady {
    fn notify(&self);
}

pub(crate) trait Cancellable: fmt::Debug {
    fn cancel(&self, error: Error);
}

impl<T> Cancellable for WaitHandle<T> {
    fn cancel(&self, error: Error) {
        self.error(error);
    }
}

impl<T> Wait<T> {
    pub(crate) fn new() -> (Self, WaitHandle<T>) {
        let (send, recv) = sync_channel(1);
        let wait = Self {
            recv,
            send,
            task: Arc::new(Mutex::new(None)),
        };
        let wait_handle = wait.handle();
        (wait, wait_handle)
    }

    fn handle(&self) -> WaitHandle<T> {
        WaitHandle {
            send: self.send.clone(),
            task: self.task.clone(),
        }
    }

    pub(crate) fn try_wait(&self) -> Option<Result<T, Error>> {
        self.recv.try_recv().ok()
    }

    pub(crate) fn wait(&self) -> Result<T, Error> {
        self.recv.recv().unwrap()
    }

    pub(crate) fn subscribe(&self, task: Box<dyn NotifyReady + Send>) {
        *self.task.lock() = Some(task);
    }

    pub(crate) fn has_subscriber(&self) -> bool {
        self.task.lock().is_some()
    }
}

impl<T> WaitHandle<T> {
    pub(crate) fn finish(&self, val: T) {
        let _ = self.send.send(Ok(val));
        self.notify();
    }

    pub(crate) fn error(&self, error: Error) {
        let _ = self.send.send(Err(error));
        self.notify();
    }

    fn notify(&self) {
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
