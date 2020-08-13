# Lapin integration with async-io

This crate integrates lapin with async-io by using async-io's reactor inside of lapin.

```
use async_lapin::*;
use lapin::{executor::Executor, Connection, ConnectionProperties, Result};
use std::{future::Future, pin::Pin};

#[derive(Debug)]
struct SmolExecutor(async_executor::Spawner);

impl Executor for SmolExecutor {
    fn spawn(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) -> Result<()> {
        self.0.spawn(f).detach();
        Ok(())
    }
}

fn main() -> Result<()> {
    smol::run(async {
        let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default().with_async_io(SmolExecutor(async_executor::Spawner::current())),
        )
        .await?; // Note the `with_async_io()` here
        let channel = conn.create_channel().await?;

        // Rest of your program
    })
}
```
