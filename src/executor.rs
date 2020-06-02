use crate::{thread::ThreadHandle, Result};
use async_task::Task;
use crossbeam_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::{
    cell::RefCell, fmt, future::Future, ops::Deref, pin::Pin, sync::Arc,
    thread::Builder as ThreadBuilder,
};

thread_local!(static LAPIN_EXECUTOR_THREAD: RefCell<bool> = RefCell::new(false));

pub(crate) fn within_executor() -> bool {
    LAPIN_EXECUTOR_THREAD.with(|executor_thread| *executor_thread.borrow())
}

fn spawn_thread(
    name: String,
    work: impl FnOnce() -> Result<()> + Send + 'static,
) -> Result<ThreadHandle> {
    Ok(ThreadHandle::new(ThreadBuilder::new().name(name).spawn(
        move || {
            LAPIN_EXECUTOR_THREAD.with(|executor_thread| *executor_thread.borrow_mut() = true);
            work()
        },
    )?))
}

pub trait Executor: std::fmt::Debug + Send + Sync {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>);
    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()>;
}

impl Executor for Arc<dyn Executor> {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        self.deref().spawn(f);
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        self.deref().spawn_blocking(f)
    }
}

#[derive(Clone)]
pub struct DefaultExecutor {
    sender: Sender<Option<Task<()>>>,
    receiver: Receiver<Option<Task<()>>>,
    threads: Arc<Mutex<Vec<ThreadHandle>>>,
}

impl DefaultExecutor {
    pub fn new(max_threads: usize) -> Result<Self> {
        let (sender, receiver) = crossbeam_channel::unbounded::<Option<Task<()>>>();
        let threads = Arc::new(Mutex::new(
            (1..=max_threads)
                .map(|id| {
                    let receiver = receiver.clone();
                    spawn_thread(format!("lapin-executor-{}", id), move || {
                        while let Ok(Some(task)) = receiver.recv() {
                            task.run();
                        }
                        Ok(())
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        ));
        Ok(Self {
            sender,
            receiver,
            threads,
        })
    }

    pub(crate) fn default() -> Result<Arc<dyn Executor>> {
        Ok(Arc::new(Self::new(1)?))
    }
}

impl fmt::Debug for DefaultExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultExecutor").finish()
    }
}

fn schedule(sender: Sender<Option<Task<()>>>, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
    let schedule = move |task| sender.send(Some(task)).expect("executor failed");
    let (task, _) = async_task::spawn(f, schedule, ());
    task.schedule();
}

impl Executor for DefaultExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
        schedule(self.sender.clone(), f);
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        let exe_sender = self.sender.clone();
        let (sender, receiver) = crossbeam_channel::bounded::<Option<Task<()>>>(1);
        let handle = spawn_thread("lapin-blocking-runner".to_string(), move || {
            if let Ok(Some(task)) = receiver.recv() {
                task.run();
            }
            Ok(())
        })?;
        schedule(
            sender,
            Box::pin(async move {
                f();
                schedule(
                    exe_sender,
                    Box::pin(async move {
                        let _ = handle.wait("blocking runner");
                    }),
                );
            }),
        );
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
