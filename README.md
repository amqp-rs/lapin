<div align="center">
<img src="logo.jpg" width="30%"></img>

[![API Docs](https://docs.rs/lapin/badge.svg)](https://docs.rs/lapin)
[![Build Status](https://travis-ci.org/sozu-proxy/lapin.svg?branch=master)](https://travis-ci.org/sozu-proxy/lapin)
[![Downloads](https://img.shields.io/crates/d/lapin.svg)](https://crates.io/crates/lapin)
[![Coverage Status](https://coveralls.io/repos/github/sozu-proxy/lapin/badge.svg?branch=master)](https://coveralls.io/github/sozu-proxy/lapin?branch=master)
[![Dependency Status](https://deps.rs/repo/github/sozu-proxy/lapin/status.svg)](https://deps.rs/repo/github/sozu-proxy/lapin)
[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

 <strong>
   A Rust AMQP client library.
 </strong>

</div>

<br />

This project follows the [AMQP 0.9.1 specifications](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf), targeting especially RabbitMQ.

## Feature switches

* `native-tls` (*default*): enable amqps support through native-tls
* `openssl`: enable amqps support through openssl (preferred over native-tls when set)
* `rustls`: enable amqps support through rustls (preferred over openssl when set, uses rustls-native-certs by default)
* `rustls-native-certs`: same as rustls, be ensure we'll still use rustls-native-certs even if the default for rustls changes
* `rustls-webpki-roots-certs`: same as rustls but using webkit-roots instead of rustls-native-certs

## Integration with async-std

Integration with async-std is provided by the [async-amqp](https://crates.io/crates/async-amqp) crate.

## Integration with smol

Integration with smol is provided by the [lapinou](https://crates.io/crates/lapinou) crate.

## Integration with tokio

Integration with tokio is provided by the [tokio-amqp](https://crates.io/crates/tokio-amqp) crate.

## Example

```rust
use futures_executor::{LocalPool, ThreadPool};
use futures_util::stream::StreamExt;
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use log::info;

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let executor = ThreadPool::new()?;

    LocalPool::new().run_until(async {
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default().with_default_executor(8),
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

        info!("Declared queue {:?}", queue);

        let mut consumer = channel_b
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        executor.spawn_ok(async move {
            info!("will consume");
            while let Some(delivery) = consumer.next().await {
                let (channel, delivery) = delivery.expect("error in consumer");
                channel
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await
                    .expect("ack");
            }
        });

        let payload = b"Hello world!";

        loop {
            let confirm = channel_a
                .basic_publish(
                    "",
                    "hello",
                    BasicPublishOptions::default(),
                    payload.to_vec(),
                    BasicProperties::default(),
                )
                .await?
                .await?;
            assert_eq!(confirm, Confirmation::NotRequested);
        }
    })
}
```
