# Lapin integration with smol

This crate integrates lapin with smol by using smol's executor inside of lapin
for its internal operations and for consumer delegates.

```
use lapinou::*;
use lapin::{Connection, ConnectionProperties, Result};

fn main() -> Result<()> {
    smol::run(async {
        let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
        let conn = Connection::connect(&addr, ConnectionProperties::default().with_smol()).await?; // Note the `with_smol()` here
        let channel = conn.create_channel().await?;

        // Rest of your program
    })
}
```
