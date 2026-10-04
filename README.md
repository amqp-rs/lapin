<div align="center">
<img src="logo.jpg" width="30%"></img>

[![API Docs](https://docs.rs/lapin/badge.svg)](https://docs.rs/lapin)
[![Build status](https://github.com/amqp-rs/lapin/workflows/Build%20and%20test/badge.svg)](https://github.com/amqp-rs/lapin/actions)
[![Downloads](https://img.shields.io/crates/d/lapin.svg)](https://crates.io/crates/lapin)
[![Coverage Status](https://coveralls.io/repos/github/amqp-rs/lapin/badge.svg?branch=main)](https://coveralls.io/github/amqp-rs/lapin?branch=main)
[![Dependency Status](https://deps.rs/repo/github/amqp-rs/lapin/status.svg)](https://deps.rs/repo/github/amqp-rs/lapin)
[![LICENSE](https://img.shields.io/crates/l/lapin)](LICENSE)

**An async AMQP 0-9-1 client library for Rust, targeting RabbitMQ.**

</div>

Lapin implements the [AMQP 0-9-1 specification](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf)
on top of an async I/O layer. It is runtime-agnostic: the same code works with
**tokio** (default), **smol**, or **async-global-executor**.

For full API documentation see [docs.rs/lapin](https://docs.rs/lapin).

When the URI does not set `frame_max`, Lapin negotiates at most a 16 MiB frame
size. Set `frame_max` in the URI to request a larger limit. I/O buffers grow as
needed up to the negotiated limit.

## Quick start

```rust,no_run
use futures_lite::stream::StreamExt;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    let addr = std::env::var("AMQP_ADDR")
        .unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;

    channel
        .queue_declare("hello".into(), QueueDeclareOptions::durable(), FieldTable::default())
        .await?;

    channel
        .basic_publish(
            "".into(),
            "hello".into(),
            BasicPublishOptions::default(),
            b"Hello, world!",
            BasicProperties::default(),
        )
        .await?
        .await?;

    let mut consumer = channel
        .basic_consume(
            "hello".into(),
            "my_consumer".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    while let Some(delivery) = consumer.next().await {
        let delivery = delivery?;
        delivery.ack(BasicAckOptions::default()).await?;
    }
    Ok(())
}
```

## Automatic connection recovery

Pass `.enable_auto_recover()` in `ConnectionProperties` to have lapin
automatically reconnect and replay exchanges, queues, bindings, and consumers
after a network failure:

```rust,no_run
use lapin::ConnectionProperties;

let props = ConnectionProperties::default().enable_auto_recover();
```

After catching a recoverable error on a channel, call
`channel.wait_for_recovery(error).await` to block until recovery is complete.

## Feature flags

### Async runtime (pick exactly one)

| Flag | Notes |
|------|-------|
| `tokio` *(default)* | Requires a running Tokio runtime |
| `smol` | Uses the smol executor |
| `async-global-executor` | Uses async-global-executor |

### TLS backend (pick at most one)

| Flag | Notes |
|------|-------|
| `rustls` *(default)* | TLS via rustls |
| `native-tls` | TLS via the platform native library |
| `openssl` | TLS via OpenSSL |

### Rustls certificate store (when `rustls` is active)

| Flag | Notes |
|------|-------|
| `rustls-platform-verifier` *(default)* | Platform trust store |
| `rustls-native-certs` | Native root certificates |
| `rustls-webpki-roots-certs` | Bundled webpki root set |

### Rustls crypto provider (at least one required)

| Flag | Notes |
|------|-------|
| `rustls--aws_lc_rs` *(default)* | Uses aws-lc-rs |
| `rustls--ring` | Uses ring (more portable) |

### Miscellaneous

| Flag | Notes |
|------|-------|
| `hickory-dns` | Hickory DNS resolver (avoids spurious network hangs) |
| `codegen` | Force protocol code regeneration at build time |
| `verbose-errors` | More detailed AMQP parser error messages |

## Custom runtimes

Lapin can use any runtime by supplying an `async_rs::Runtime` value:

```rust,no_run
use lapin::{Connection, ConnectionProperties, Result};

async fn connect_with_custom_runtime() -> Result<()> {
    let runtime = async_rs::Runtime::tokio_current();
    let conn = Connection::connect_with_runtime(
        "amqp://localhost",
        ConnectionProperties::default(),
        runtime,
    ).await?;
    drop(conn);
    Ok(())
}
```

See [async-rs](https://docs.rs/async-rs) for available runtime wrappers.
