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

- unstable: enable access to the experimental reconnection features
- codegen: force code generation (default to pregenerated sources)
- vendored-openssl: use a vendored openssl version instead of the system one (when using openssl backend)
- verbose-errors: enable more verbose errors in the AMQP parser

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

Lapin can use any runtime of your choice by passing it to the `ConnectionProperties`.

You can configure the executor to use through [executor-trait](https://crates.io/crates/executor-trait).

You can configure the reactor to use through [reactor-trait](https://crates.io/crates/reactor-trait).

There are implementations for tokio, async-std and others.

## Experimental auomatic reconnection

WARNING: use at your own risk, this feature is not considered stable yet. Expect some bugs and please help by reporting them.

There is experimental support for recovering connection after errors. For now, only Channels can be recovered after an AMQP soft error. Connection is next in TODO.

To enable this, you need to enable the `unstable` feature for lapin, and add it to the `ConnectionProperties`:

```rust
let recovery_config = RecoveryConfig::default().auto_recover_channels();
let properties = ConnectionProperties::default().with_experimental_recovery_config(recovery_config);
// connect using properties.
```

You can then check if an error can be recovered and wait for recovery (some new syntax on top of this will come soon):

```rust
if err.is_amqp_soft_error() {
    if let Some(notifier) = err.notifier() {
        notifier.await?;
    }
}
```

## Example

```rust
use futures_lite::stream::StreamExt;
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use tracing::info;

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    async_global_executor::block_on(async {
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default(),
        )
        .await?;

        info!("CONNECTED");

        let channel_a = conn.create_channel().await?;
        let channel_b = conn.create_channel().await?;

        let queue = channel_a
            .queue_declare(
                "hello",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        info!(?queue, "Declared queue");

        let mut consumer = channel_b
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        async_global_executor::spawn(async move {
            info!("will consume");
            while let Some(delivery) = consumer.next().await {
                let delivery = delivery.expect("error in consumer");
                delivery
                    .ack(BasicAckOptions::default())
                    .await
                    .expect("ack");
            }
        }).detach();

        let payload = b"Hello world!";

        loop {
            let confirm = channel_a
                .basic_publish(
                    "",
                    "hello",
                    BasicPublishOptions::default(),
                    payload,
                    BasicProperties::default(),
                )
                .await?
                .await?;
            assert_eq!(confirm, Confirmation::NotRequested);
        }
    })
}
```
