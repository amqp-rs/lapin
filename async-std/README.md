# Lapin integration with async-std

This crate integrates lapin with async-std by using async-std's executor inside of lapin
for its internal operations and for consumer delegates.

```
use async_amqp::*;
use lapin::{Connection, ConnectionProperties, Result};

#[async_std::main]
async fn main() -> Result<()> {
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default().with_async_std()).await?; // Note the `with_async_std()` here
    let channel = conn.create_channel().await?;

    // Rest of your program
}
```
