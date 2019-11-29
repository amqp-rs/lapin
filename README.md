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

## Example

> **Note**: To use async/await, enable the `futures` feature in your Cargo.toml.

```rust
use lapin::{
    options::*, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties
};
use futures::{future::FutureExt, stream::StreamExt};
use log::info;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    let conn = Connection::connect(&addr, ConnectionProperties::default())
	.await
	.expect("Couldn't connect to RabbitMQ.");

    info!("CONNECTED");

    let channel_a = conn.create_channel().await?;
    let channel_b = conn.create_channel().await?;

    channel_a
        .queue_declare(
            "my_first_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let queue = channel_b
        .queue_declare(
            "my_second_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let consumer = channel_b
        .clone()
        .basic_consume(
            &queue,
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    tokio::spawn(async move {
        info!("Consuming from Channel B...");

        consumer
            .for_each(move |delivery| {
                let delivery = delivery.expect("Couldn't receive delivery from RabbitMQ.");
                channel_b
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .map(|_| ())
            })
            .await
    });

    let payload = b"Hello world!";

    loop {
        channel_a
            .basic_publish(
                "",
                "my_first_queue",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .await?;
    }
}
```
