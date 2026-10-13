#![cfg(feature = "tokio")]

//! Checks that a closed connection leaves no task behind.
//!
//! Runs against a real broker. The connections ask for a one-second
//! heartbeat, so the heartbeat task of each closed connection wakes up and
//! ends well within the settling time.

use lapin::{Connection, ConnectionProperties};
use std::time::Duration;

const CONNECTIONS: usize = 20;
/// Longer than the heartbeat interval, so every task a closed connection
/// leaves running has had time to notice the close and end.
const SETTLE: Duration = Duration::from_secs(3);

#[tokio::test(flavor = "multi_thread")]
async fn closed_connections_leave_no_tasks_behind() {
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let separator = if addr.contains('?') { '&' } else { '?' };
    let addr = format!("{addr}{separator}heartbeat=1");
    let metrics = tokio::runtime::Handle::current().metrics();
    let before = metrics.num_alive_tasks();

    for _ in 0..CONNECTIONS {
        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .unwrap();
        conn.close(200, "bye".into()).await.unwrap();
    }
    tokio::time::sleep(SETTLE).await;

    assert_eq!(metrics.num_alive_tasks(), before);
}
