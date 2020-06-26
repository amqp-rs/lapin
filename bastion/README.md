# Lapin integration with bastion

This crate integrates lapin with bastin by using bastion's executor inside of lapin
for its internal operations and for consumer delegates.

```
use bastion_amqp::*;
use bastion::run;
use lapin::{Connection, ConnectionProperties, Result};

pub async fn consumer(addr: String) -> Result<()> {
    let conn = Connection::connect(&addr, ConnectionProperties::default().with_bastion()).await?; // Note the `with_bastion()` here
    let channel = conn.create_channel().await?;
    // Rest of your program
}

fn main() -> Result<()> {
    Bastion::init();
    Bastion::start();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    run!(consumer(addr))
}
```
