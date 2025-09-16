use async_rs::{Runtime, traits::*};
use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties, Result, options::*,
    types::FieldTable,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let runtime = Runtime::tokio_current();

    let conn = Connection::connect_with_runtime(
        &addr,
        ConnectionProperties::default().with_connection_name("pubsub-example".into()),
        runtime.clone(),
    )
    .await?;

    info!("CONNECTED");

    let channel_a = conn.create_channel().await?;
    let channel_b = conn.create_channel().await?;

    let queue = channel_a
        .queue_declare(
            "hello".into(),
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!(?queue, "Declared queue");

    let mut consumer = channel_b
        .basic_consume(
            "hello".into(),
            "my_consumer".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    let cons = runtime.spawn(async move {
        info!("will consume");
        while let Some(delivery) = consumer.next().await {
            let delivery = delivery?;
            delivery.ack(BasicAckOptions::default()).await?;
        }
        Ok(())
    });

    let payload = b"Hello world!";

    for _ in 0..1500000 {
        let confirm = channel_a
            .basic_publish(
                "".into(),
                "hello".into(),
                BasicPublishOptions::default(),
                payload,
                BasicProperties::default(),
            )
            .await?
            .await?;
        assert_eq!(confirm, Confirmation::NotRequested);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    channel_b
        .basic_cancel("my_consumer".into(), BasicCancelOptions::default())
        .await?;
    cons.await
}
