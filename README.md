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

This project follows the [AMQP 0.9.1 specifications](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf), targetting especially RabbitMQ.

## Feature switches

* `native-tls` (*default*): enable amqps support through native-tls
* `openssl`: enable amqps support through openssl (preferred over native-tls when set)
* `rustls`: enable amqps support through rustls (preferred over openssl when set, uses rustls-native-certs by default)
* `rustls-native-certs`: same as rustls, be ensure we'll still use rustls-native-certs even if the default for rustls changes
* `rustls-webpki-roots-certs`: same as rustls but using webkit-roots instead of rustls-native-certs

## Example

```rust
use futures_executor::LocalPool;
use futures_util::{future::FutureExt, stream::StreamExt, task::LocalSpawnExt};
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use log::info;

fn main() -> Result<()> {
    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let mut executor = LocalPool::new();
    let spawner = executor.spawner();

    executor.run_until(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;

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

        let consumer = channel_b
            .clone()
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        let _consumer = spawner.spawn_local(async move {
            info!("will consume");
            consumer
                .for_each(move |delivery| {
                    let delivery = delivery.expect("error caught in in consumer");
                    channel_b
                        .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                        .map(|_| ())
                })
                .await
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
