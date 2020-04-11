use lapin::{
    executor::Executor,
    Result,
};

#[derive(Debug)]
pub struct TokioExecutor;

impl Executor for TokioExecutor {
    fn execute(&self, f: Box<dyn FnOnce() + Send>) -> Result<()> {
        tokio::spawn(async {
            f()
        });
        Ok(())
    }
}
