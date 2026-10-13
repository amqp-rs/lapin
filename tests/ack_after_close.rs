#![cfg(feature = "tokio")]

//! Checks that acking a delivery after its connection was closed completes.
//!
//! Runs against a real broker. Once the connection is closed, its IO loop no
//! longer runs commands, so the ack never reaches the broker; its promise must
//! still complete, with an error, instead of staying pending forever.

use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Connection, ConnectionProperties,
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn ack_after_connection_close_completes() {
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default())
        .await
        .unwrap();
    let channel = conn.create_channel().await.unwrap();
    let queue = channel
        .queue_declare(
            "".into(),
            QueueDeclareOptions {
                exclusive: true,
                ..QueueDeclareOptions::default()
            },
            FieldTable::default(),
        )
        .await
        .unwrap();
    channel
        .basic_publish(
            "".into(),
            queue.name().clone(),
            BasicPublishOptions::default(),
            b"hello",
            BasicProperties::default(),
        )
        .await
        .unwrap()
        .await
        .unwrap();
    let mut consumer = channel
        .basic_consume(
            queue.name().clone(),
            "ack_after_close".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let delivery = tokio::time::timeout(TIMEOUT, consumer.next())
        .await
        .expect("no delivery")
        .expect("consumer ended")
        .unwrap();

    conn.close(200, "bye".into()).await.unwrap();

    let ack = delivery.acker.ack(BasicAckOptions::default());
    let result = tokio::time::timeout(TIMEOUT, ack)
        .await
        .expect("the ack never completed");
    assert!(result.is_err(), "{result:?}");
}
