use crate::{internal_rpc::InternalRPCHandle, Promise, Result};
use async_task::Task;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::{
    fmt,
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::Arc,
    thread::{Builder as ThreadBuilder, JoinHandle},
};

pub trait Executor: std::fmt::Debug + Send + Sync {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()>;
}

pub(crate) trait ExecutorExt: Executor {
    fn spawn_internal(&self, promise: Promise<()>, internal_rpc: InternalRPCHandle) -> Result<()> {
        if let Some(res) = promise.try_wait() {
            if let Err(err) = res.clone() {
                internal_rpc.set_connection_error(err);
            }
            res
        } else {
            self.spawn(Box::pin(async move {
                if let Err(err) = promise.await {
                    internal_rpc.set_connection_error(err);
                }
            }))
        }
    }
}

impl Executor for Arc<dyn Executor> {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        self.deref().spawn(f)
    }
}
impl ExecutorExt for Arc<dyn Executor> {}

#[derive(Clone)]
pub struct DefaultExecutor {
    sender: Sender<Task<()>>,
    receiver: Receiver<Task<()>>,
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
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
            threads.push(
                ThreadBuilder::new()
                    .name(format!("executor {}", id))
                    .spawn(move || {
                        for task in receiver {
                            task.run();
                        }
                    })?,
            );
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
        let schedule = move |task| sender.send(task).expect("executor failed");
        let (task, _) = async_task::spawn(f, schedule, ());
        task.schedule();
        Ok(())
    }
}
