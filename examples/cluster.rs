//! Connect to a RabbitMQ cluster, failing over to another node when the one we're
//! connected to goes away.
//!
//! The failover itself works with any AMQP 0-9-1 broker, but this example declares a
//! quorum queue, which is RabbitMQ specific.
//!
//! Set `AMQP_ADDRS` to a comma separated list of node URIs, e.g.
//! `AMQP_ADDRS=amqp://rabbit1:5672/%2f,amqp://rabbit2:5672/%2f,amqp://rabbit3:5672/%2f`

use async_rs::{Runtime, TokioRuntime, traits::*};
use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties, Result,
    options::*,
    types::{AMQPValue, FieldTable},
};
use tracing::info;

async fn rabbit_stuff(addrs: Vec<String>, runtime: TokioRuntime) -> Result<()> {
    let conn = Connection::connect_cluster_with_runtime(
        &addrs,
        ConnectionProperties::default().enable_auto_recover(),
        runtime.clone(),
    )
    .await?;

    info!("CONNECTED");

    let channel_a = conn.create_channel().await?;
    let channel_b = conn.create_channel().await?;

    // RabbitMQ quorum queues are replicated across the cluster nodes, so they survive the loss
    // of the node we're connected to.
    let mut arguments = FieldTable::default();
    arguments.insert(
        "x-queue-type".into(),
        AMQPValue::LongString("quorum".into()),
    );
    let queue = channel_a
        .queue_declare(
            "hello-cluster".into(),
            QueueDeclareOptions::durable(),
            arguments,
        )
        .await?;

    info!(?queue, "Declared queue");

    let mut consumer = channel_b
        .basic_consume(
            "hello-cluster".into(),
            "my_consumer".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    runtime.spawn(async move {
        while let Some(delivery) = consumer.next().await {
            let delivery = match delivery {
                Ok(delivery) => delivery,
                Err(err) => {
                    channel_b.wait_for_recovery(err).await?;
                    continue;
                }
            };
            delivery.ack(BasicAckOptions::default()).await?;
        }
        Ok::<(), lapin::Error>(())
    });

    loop {
        match publish(&channel_a, b"Hello cluster!").await {
            Ok(confirm) => assert_eq!(confirm, Confirmation::NotRequested),
            Err(err) => channel_a.wait_for_recovery(err).await?,
        }
        runtime.sleep(std::time::Duration::from_millis(100)).await;
    }
}

async fn publish(channel: &Channel, payload: &'static [u8]) -> Result<Confirmation> {
    channel
        .basic_publish(
            "".into(),
            "hello-cluster".into(),
            BasicPublishOptions::default(),
            payload,
            BasicProperties::default(),
        )
        .await?
        .await
}

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addrs = std::env::var("AMQP_ADDRS")
        .unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f?connection_timeout=5000".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    let runtime = Runtime::tokio()?;

    runtime.clone().block_on(rabbit_stuff(addrs, runtime))
}
