use crate::{thread::ThreadHandle, Result};
use async_task::Task;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::{fmt, future::Future, ops::Deref, pin::Pin, sync::Arc, thread::Builder as ThreadBuilder};

pub trait Executor: std::fmt::Debug + Send + Sync {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()>;
}

impl Executor for Arc<dyn Executor> {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        self.deref().spawn(f)
    }
}

#[derive(Clone)]
pub struct DefaultExecutor {
    sender: Sender<Option<Task<()>>>,
    receiver: Receiver<Option<Task<()>>>,
    threads: Arc<Mutex<Vec<ThreadHandle>>>,
    max_threads: usize,
}

impl DefaultExecutor {
    pub fn new(max_threads: usize) -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let threads = Default::default();
        Self {
            sender,
            receiver,
            threads,
            max_threads,
        }
    }

    pub(crate) fn maybe_spawn_thread(&self) -> Result<()> {
        let mut threads = self.threads.lock();
        let id = threads.len() + 1;
        if id <= self.max_threads {
            let receiver = self.receiver.clone();
            threads.push(ThreadHandle::new(
                ThreadBuilder::new()
                    .name(format!("executor {}", id))
                    .spawn(move || {
                        while let Ok(Some(task)) = receiver.recv() {
                            task.run();
                        }
                        Ok(())
                    })?,
            ));
        }
        Ok(())
    }
}

impl Default for DefaultExecutor {
    fn default() -> Self {
        Self::new(1)
    }
}

impl fmt::Debug for DefaultExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultExecutor")
            .field("max_threads", &self.max_threads)
            .finish()
    }
}

impl Executor for DefaultExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        self.maybe_spawn_thread()?;
        let sender = self.sender.clone();
        let schedule = move |task| sender.send(Some(task)).expect("executor failed");
        let (task, _) = async_task::spawn(f, schedule, ());
        task.schedule();
        Ok(())
    }
}

impl Drop for DefaultExecutor {
    fn drop(&mut self) {
        if let Some(threads) = self.threads.try_lock() {
            for _ in threads.iter() {
                let _ = self.sender.send(None);
            }
            for thread in threads.iter() {
                if !thread.is_current() {
                    let _ = thread.wait("executor");
                }
            }
        }
    }
}
