# Lapin integration with async-global-executor

This crate integrates lapin with async-global-executor by using its executor inside of lapin
for its internal operations and for consumer delegates.

```
use lapin_async_global_executor::*;
use lapin::{Connection, ConnectionProperties, Result};

fn main() -> Result<()> {
    async_global_executor::run(async {
        let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
        let conn = Connection::connect(&addr, ConnectionProperties::default().with_async_global_executor()).await?; // Note the `with_async_global_executor()` here
        let channel = conn.create_channel().await?;

        // Rest of your program
    })
}
```
