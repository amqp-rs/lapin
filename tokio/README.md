# Lapin integration with tokio

This crate integrates lapin with tokio by using tokio's executor inside of lapin
for its internal operations and for consumer delegates.

```
use tokio_amqp::*;
use lapin::{Connection, ConnectionProperties, Result};
use std::sync::Arc;
use tokio::runtime::Runtime;

async fn tokio_main(rt: Arc<Runtime>) -> Result<()> {
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default().with_tokio(rt)).await?;
    let channel = conn.create_channel().await?;

    // Rest of your program
}

fn main() {
    let rt = Arc::new(Runtime::new().expect("failed to create runtime"));
    rt.block_on(tokio_main(rt.clone())).expect("error");
}
```
