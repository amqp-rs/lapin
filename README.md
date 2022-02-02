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

## Feature switches

* `codegen`: generate code instead of using pregenerated one
* `native-tls`: enable amqps support through native-tls
* `openssl`: enable amqps support through openssl (preferred over native-tls when set)
* `rustls` (*default*): enable amqps support through rustls (preferred over openssl when set, uses rustls-native-certs by default)
* `rustls-native-certs`: same as rustls, be ensure we'll still use rustls-native-certs even if the default for rustls changes
* `rustls-webpki-roots-certs`: same as rustls but using webkit-roots instead of rustls-native-certs

## Integration with third-party runtimes

Lapin can use any runtime of your choice by passing it to the `ConnectionProperties`.

You can configure the executor to use through [executor-trait](https://crates.io/crates/executor-trait).

You can configure the reactor to use through [reactor-trait](https://crates.io/crates/reactor-trait).

There are implementations for tokio, async-std and others.

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
