#![cfg(feature = "tokio")]

//! Checks that a request still waiting for its reply when the broker closes
//! the channel fails with the broker's error instead of staying pending.
//!
//! Runs against a real broker. A passive declare of a missing queue is
//! answered with a channel close (404 NOT_FOUND) instead of a declare-ok; the
//! declare's promise is only rejected once the closed channel is removed.

use lapin::{
    Connection, ConnectionProperties, ErrorKind, options::QueueDeclareOptions, types::FieldTable,
};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn passive_declare_of_a_missing_queue_fails() {
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default())
        .await
        .unwrap();
    let channel = conn.create_channel().await.unwrap();

    let declare = channel.queue_declare(
        "lapin-test-queue-that-does-not-exist".into(),
        QueueDeclareOptions {
            passive: true,
            ..QueueDeclareOptions::default()
        },
        FieldTable::default(),
    );
    let error = tokio::time::timeout(TIMEOUT, declare)
        .await
        .expect("the declare never completed")
        .unwrap_err();
    assert!(
        matches!(error.kind(), ErrorKind::ProtocolError(_)),
        "{error}"
    );

    conn.close(200, "bye".into()).await.unwrap();
}
