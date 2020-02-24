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
    message::DeliveryResult, options::*, types::FieldTable, BasicProperties, Channel, Connection,
    ConnectionProperties, ConsumerDelegate,
};
use log::info;

#[derive(Clone, Debug)]
struct Subscriber {
    channel: Channel,
}

impl ConsumerDelegate for Subscriber {
    fn on_new_delivery(&self, delivery: DeliveryResult) {
        if let Ok(Some(delivery)) = delivery {
            self.channel
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .wait()
                .expect("basic_ack");
        }
    }
}

fn main() {
    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default())
        .wait()
        .expect("connection error");

    info!("CONNECTED");

    let channel_a = conn.create_channel().wait().expect("create_channel");
    let channel_b = conn.create_channel().wait().expect("create_channel");

    channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("queue_declare");
    let queue = channel_b
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("queue_declare");

    info!("will consume");
    channel_b
        .clone()
        .basic_consume(
            &queue,
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("basic_consume")
        .set_delegate(Box::new(Subscriber { channel: channel_b }));

    let payload = b"Hello world!";

    loop {
        channel_a
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .wait()
            .expect("basic_publish");
    }
}
```
