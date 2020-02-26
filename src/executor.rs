use crate::{Error, Result};
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::{
    sync::Arc,
    thread::{Builder as ThreadBuilder, JoinHandle},
};

pub trait Executor: std::fmt::Debug + Send + Sync {
    fn execute(&self, f: Box<dyn FnOnce() + Send>) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct DefaultExecutor {
    sender: Sender<Box<dyn FnOnce() + Send>>,
    receiver: Receiver<Box<dyn FnOnce() + Send>>,
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    max_threads: usize,
}

impl DefaultExecutor {
    pub fn new(max_threads: usize) -> Arc<Self> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Arc::new(Self {
            sender,
            receiver,
            threads: Default::default(),
            max_threads,
        })
    }

    pub fn default() -> Arc<Self> {
        Self::new(1)
    }
}

impl DefaultExecutor {
    pub(crate) fn maybe_spawn_thread(&self) -> Result<()> {
        let mut threads = self.threads.lock();
        let id = threads.len() + 1;
        if id <= self.max_threads {
            let receiver = self.receiver.clone();
            threads.push(
                ThreadBuilder::new()
                    .name(format!("executor {}", id))
                    .spawn(move || {
                        for f in receiver {
                            f();
                        }
                    })
                    .map_err(Arc::new)
                    .map_err(Error::IOError)?,
            );
        }
        Ok(())
    }
}

impl Executor for DefaultExecutor {
    fn execute(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        self.maybe_spawn_thread()?;
        self.sender.send(f).expect("executor failed");
        Ok(())
    }
}
