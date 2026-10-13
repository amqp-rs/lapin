#![cfg(feature = "tokio")]

//! Checks that acking a delivery after its channel or connection was closed
//! completes with `Ok(false)`, as the `Acker` docs promise: the close poisons
//! the delivery's acker, so the ack is never sent.
//!
//! A connection closes its channels on the internal RPC loop shortly after
//! `Connection::close` returns, so the connection case acks once the close has
//! settled and the IO loop is gone. That is where the ack used to hang.
//!
//! An ack already under way when the broker closes the channel fails with the
//! error the broker closed it with.
//!
//! Runs against a real broker.

use futures_lite::stream::StreamExt;
use lapin::{
    Acker, BasicProperties, Channel, Connection, ConnectionProperties, ErrorKind, Queue,
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions},
    protocol::{AMQPErrorKind, AMQPSoftError},
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
/// must complete, none may be sent once one was not, any that fails reports the
/// broker's error, and every acker ends up poisoned.
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
    let declared = declare_missing_queue(channel).await;
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
    assert_failures_report_missing_queue(&results);
    assert!(deliveries.iter().chain(&racing).all(|d| d.acker.poisoned()));

    conn.close(200, "bye".into()).await.unwrap();
}

/// Acks run in large concurrent bursts while an invalid operation on the same
/// channel makes the broker close it, so many of them can be under way when the
/// close lands. Each one must complete, and any that fails reports the broker's
/// error.
#[tokio::test(flavor = "multi_thread")]
async fn concurrent_acks_cut_short_by_a_broker_channel_close_report_its_error() {
    const DELIVERIES: usize = 3000;
    const BATCH: usize = 1000;
    const BATCHES_BEFORE_CLOSE: usize = 1;

    let (conn, channel, deliveries) = receive(DELIVERIES).await;
    let mut declare = None;
    let mut results = Vec::with_capacity(DELIVERIES);
    for (index, batch) in deliveries.chunks(BATCH).enumerate() {
        if index == BATCHES_BEFORE_CLOSE {
            declare = Some(tokio::spawn(declare_missing_queue(channel.clone())));
        }
        let acks = batch
            .iter()
            .map(|delivery| delivery.acker.clone())
            .map(|acker| tokio::spawn(async move { ack(&acker).await }))
            .collect::<Vec<_>>();
        for ack in acks {
            results.push(ack.await.unwrap());
        }
    }
    let declared = declare.expect("the declare never started").await.unwrap();
    assert!(declared.is_err(), "{declared:?}");

    assert!(
        results.iter().any(|result| !matches!(result, Ok(true))),
        "the close never cut into the acks"
    );
    assert_failures_report_missing_queue(&results);

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

/// Passively declares a queue that does not exist, which makes the broker
/// close the channel with `NOT_FOUND`.
async fn declare_missing_queue(channel: Channel) -> lapin::Result<Queue> {
    let declare = channel.queue_declare(
        "lapin-test-queue-that-does-not-exist".into(),
        QueueDeclareOptions {
            passive: true,
            ..QueueDeclareOptions::default()
        },
        FieldTable::default(),
    );
    tokio::time::timeout(TIMEOUT, declare)
        .await
        .expect("the declare never completed")
}

/// Asserts that every failed ack reports the `NOT_FOUND` the broker closed the
/// channel with.
fn assert_failures_report_missing_queue(results: &[lapin::Result<bool>]) {
    let other_failures = results
        .iter()
        .filter_map(|result| result.as_ref().err())
        .filter(|error| {
            !matches!(
                error.kind(),
                ErrorKind::ProtocolError(error)
                    if *error.kind() == AMQPErrorKind::Soft(AMQPSoftError::NOTFOUND)
            )
        })
        .collect::<Vec<_>>();
    assert!(
        other_failures.is_empty(),
        "acks failed with another error than the broker's: {other_failures:?}"
    );
}

async fn ack(acker: &Acker) -> lapin::Result<bool> {
    let ack = acker.ack(BasicAckOptions::default());
    tokio::time::timeout(TIMEOUT, ack)
        .await
        .expect("the ack never completed")
}
