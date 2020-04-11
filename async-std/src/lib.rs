use lapin::{
    executor::Executor,
    Result,
};

#[derive(Debug)]
pub struct AsyncStdExecutor;

impl Executor for AsyncStdExecutor {
    fn execute(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        async_std::task::spawn(async {
            f();
        });
        Ok(())
    }
}
