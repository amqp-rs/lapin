# Lapin integration with async-io

This crate integrates lapin with async-io by using async-io's reactor inside of lapin.

```
use async_lapin::*;
use lapin::{Connection, ConnectionProperties, Result};

fn main() -> Result<()> {
    blocking::block_on(async {
        let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default().with_async_io(|fut| smol::Task::spawn(fut).detach()),
        )
        .await?; // Note the `with_async_io()` here
        let channel = conn.create_channel().await?;

        // Rest of your program
    })
}
```
