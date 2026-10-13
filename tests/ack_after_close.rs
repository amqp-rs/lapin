#![cfg(feature = "tokio")]

//! Checks that acking a delivery after its channel or connection was closed
//! completes with `Ok(false)`, as the `Acker` docs promise: the close poisons
//! the delivery's acker, so the ack is never sent.
//!
//! A connection closes its channels on the internal RPC loop shortly after
//! `Connection::close` returns, so the connection case acks once the close has
//! settled and the IO loop is gone. That is where the ack used to hang.
//!
//! Runs against a real broker.

use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties,
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);
/// How long a closed connection is given to finish shutting down.
const SETTLE: Duration = Duration::from_secs(1);

#[tokio::test]
async fn ack_after_channel_close_is_not_sent() {
    let (conn, channel, delivery) = receive_one().await;
    channel.close(200, "bye".into()).await.unwrap();
    assert!(delivery.acker.poisoned());
    assert!(!delivery.acker.usable());
    let result = ack(&delivery).await;
    assert!(matches!(result, Ok(false)), "{result:?}");
    conn.close(200, "bye".into()).await.unwrap();
}

#[tokio::test]
async fn ack_after_connection_close_is_not_sent() {
    let (conn, _channel, delivery) = receive_one().await;
    conn.close(200, "bye".into()).await.unwrap();
    tokio::time::sleep(SETTLE).await;
    assert!(delivery.acker.poisoned());
    assert!(!delivery.acker.usable());
    let result = ack(&delivery).await;
    assert!(matches!(result, Ok(false)), "{result:?}");
}

/// Acks keep going while an invalid operation on the same channel makes the
/// broker close it: acks land before, around and after the close. Each one
/// must complete, none may be sent once one was not, and every acker ends up
/// poisoned.
#[tokio::test(flavor = "multi_thread")]
async fn acks_racing_a_broker_channel_close_all_complete() {
    const DELIVERIES: usize = 1000;
    const ACKED_FIRST: usize = 10;

    let (conn, channel, mut deliveries) = receive(DELIVERIES).await;
    let racing = deliveries.split_off(ACKED_FIRST);
    for delivery in &deliveries {
        let result = ack(delivery).await;
        assert!(matches!(result, Ok(true)), "{result:?}");
    }

    let acks = tokio::spawn(async move {
        let mut results = Vec::with_capacity(racing.len());
        for delivery in &racing {
            results.push(ack(delivery).await);
        }
        (racing, results)
    });
    let declare = channel.queue_declare(
        "lapin-test-queue-that-does-not-exist".into(),
        QueueDeclareOptions {
            passive: true,
            ..QueueDeclareOptions::default()
        },
        FieldTable::default(),
    );
    let declared = tokio::time::timeout(TIMEOUT, declare)
        .await
        .expect("the declare never completed");
    assert!(declared.is_err(), "{declared:?}");
    let (racing, results) = acks.await.unwrap();

    let unsent = results
        .iter()
        .position(|result| !matches!(result, Ok(true)));
    let unsent = unsent.expect("the close never cut into the acks");
    assert!(
        results[unsent..]
            .iter()
            .all(|result| !matches!(result, Ok(true))),
        "an ack was sent after an earlier one was not: {results:?}"
    );
    assert!(deliveries.iter().chain(&racing).all(|d| d.acker.poisoned()));

    conn.close(200, "bye".into()).await.unwrap();
}

async fn receive_one() -> (Connection, Channel, Delivery) {
    let (conn, channel, mut deliveries) = receive(1).await;
    (conn, channel, deliveries.remove(0))
}

/// Publishes `count` messages to a fresh exclusive queue and receives them unacked.
async fn receive(count: usize) -> (Connection, Channel, Vec<Delivery>) {
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
    for _ in 0..count {
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
    }
    let mut consumer = channel
        .basic_consume(
            queue.name().clone(),
            "ack_after_close".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    let mut deliveries = Vec::with_capacity(count);
    while deliveries.len() < count {
        let delivery = tokio::time::timeout(TIMEOUT, consumer.next())
            .await
            .expect("no delivery")
            .expect("consumer ended")
            .unwrap();
        deliveries.push(delivery);
    }
    (conn, channel, deliveries)
}

async fn ack(delivery: &Delivery) -> lapin::Result<bool> {
    let ack = delivery.acker.ack(BasicAckOptions::default());
    tokio::time::timeout(TIMEOUT, ack)
        .await
        .expect("the ack never completed")
}
