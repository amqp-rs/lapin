<div align="center">
<img src="logo.jpg" width="30%"></img>

[![API Docs](https://docs.rs/lapin/badge.svg)](https://docs.rs/lapin)
[![Build status](https://github.com/amqp-rs/lapin/workflows/Build%20and%20test/badge.svg)](https://github.com/amqp-rs/lapin/actions)
[![Downloads](https://img.shields.io/crates/d/lapin.svg)](https://crates.io/crates/lapin)
[![Coverage Status](https://coveralls.io/repos/github/amqp-rs/lapin/badge.svg?branch=main)](https://coveralls.io/github/amqp-rs/lapin?branch=main)
[![Dependency Status](https://deps.rs/repo/github/amqp-rs/lapin/status.svg)](https://deps.rs/repo/github/amqp-rs/lapin)
[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

 <strong>
   A Rust AMQP client library.
 </strong>

</div>

<br />

This project follows the [AMQP 0.9.1 specifications](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf), targeting especially RabbitMQ.

## Features

- hickory-dns: use hickory-dns for domain name resolution to avoid spurious network hangs
- codegen: force code generation (default to pregenerated sources)
- vendored-openssl: use a vendored openssl version instead of the system one (when using openssl backend)
- verbose-errors: enable more verbose errors in the AMQP parser

## Runtime

- tokio (default)
- smol
- async-global-executor

## TLS backends

- native-tls
- openssl
- rustls (default)

## Rustls certificates store

- rustls-native-certs (default)
- rustls-webpki-roots-certs

## Warning about crypto backends for rustls

A crypto implementation must be enabled in rustls using feature flags.
We mimic what rustls does, providing one feature flag per implementation and enabling the same as rustls by default.
Available options are:
- `rustls--aws_lc_rs` (default)
- `rustls--ring`

## Integration with third-party runtimes

Lapin can use any runtime of your choice by passing an `async_rs::Runtime` when connecting.

There are implementations for tokio, smol and others in [async-rs](https://docs.rs/async-rs)

## Automatic connection recovery (on e.g. network failure)

There is support for recovering connection after errors. To enable this, you need to enable it in the `ConnectionProperties`:

```rust
let properties = ConnectionProperties::default().enable_auto_recover().configure_backoff(|backoff| {
    backoff.with_max_times(3) // It is recommended to configure at least this when enabling recovery to also retry the TCP connection when it fails.
});
// connect using properties.
```

You can then check if an error can be recovered and wait for recovery:

```rust
channel.wait_for_recovery(error).await?;
```

## Example

```rust
use async_rs::{Runtime, traits::*};
use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties, Result, options::*,
    types::FieldTable,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let runtime = Runtime::tokio_current();

    let conn = Connection::connect_with_runtime(
        &addr,
        ConnectionProperties::default().with_connection_name("pubsub-example".into()),
        runtime.clone(),
    )
    .await?;

    info!("CONNECTED");

    let channel_a = conn.create_channel().await?;
    let channel_b = conn.create_channel().await?;

    let queue = channel_a
        .queue_declare(
            "hello".into(),
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!(?queue, "Declared queue");

    let mut consumer = channel_b
        .basic_consume(
            "hello".into(),
            "my_consumer".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    let cons = runtime.spawn(async move {
        info!("will consume");
        while let Some(delivery) = consumer.next().await {
            let delivery = delivery?;
            delivery.ack(BasicAckOptions::default()).await?;
        }
        Ok(())
    });

    let payload = b"Hello world!";

    for _ in 0..1500000 {
        let confirm = channel_a
            .basic_publish(
                "".into(),
                "hello".into(),
                BasicPublishOptions::default(),
                payload,
                BasicProperties::default(),
            )
            .await?
            .await?;
        assert_eq!(confirm, Confirmation::NotRequested);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    channel_b
        .basic_cancel("my_consumer".into(), BasicCancelOptions::default())
        .await?;
    cons.await
}
```
