use async_rs::{Runtime, TokioRuntime, traits::*};
use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties, Result, options::*,
    types::FieldTable,
};
use tracing::info;

async fn rabbit_stuff(addr: String, runtime: TokioRuntime) -> Result<()> {
    let conn = Connection::connect_with_runtime(
        &addr,
        ConnectionProperties::default().enable_auto_recover(),
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
    runtime.spawn(async move {
        info!("will consume");
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

    let payload = b"Hello world!";

    loop {
        match publish(&channel_a, payload).await {
            Ok(confirm) => assert_eq!(confirm, Confirmation::NotRequested),
            Err(err) => channel_a.wait_for_recovery(err).await?,
        }
    }
}

async fn publish(channel: &Channel, payload: &'static [u8]) -> Result<Confirmation> {
    channel
        .basic_publish(
            "".into(),
            "hello".into(),
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

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let runtime = Runtime::tokio()?;

    runtime.clone().block_on(rabbit_stuff(addr, runtime))
}
