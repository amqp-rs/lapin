use async_rs::{AGERuntime, Runtime, traits::*};
use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties, Result, options::*,
    types::FieldTable,
};
use tracing::info;

fn retry_rabbit_stuff(addr: String, runtime: AGERuntime) {
    std::thread::sleep(std::time::Duration::from_millis(2000));
    tracing::debug!("Reconnecting to rabbitmq");
    try_rabbit_stuff(addr, runtime);
}

fn try_rabbit_stuff(addr: String, runtime: AGERuntime) {
    runtime.clone().spawn(async move {
        if let Err(err) = rabbit_stuff(addr.clone(), runtime.clone()).await {
            tracing::error!("Error: {}", err);
            retry_rabbit_stuff(addr, runtime);
        }
    });
}

async fn rabbit_stuff(addr: String, runtime: AGERuntime) -> Result<()> {
    let conn =
        Connection::connect_with_runtime(&addr, ConnectionProperties::default(), runtime.clone())
            .await?;

    info!("CONNECTED");

    let channel_a = conn.create_channel().await?;
    let channel_b = conn.create_channel().await?;

    let queue = channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!(?queue, "Declared queue");

    let mut consumer = channel_b
        .basic_consume(
            "hello",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    runtime.spawn(async move {
        info!("will consume");
        while let Some(delivery) = consumer.next().await {
            let delivery = delivery?;
            delivery.ack(BasicAckOptions::default()).await?;
        }
        Ok::<(), lapin::Error>(())
    });

    let payload = b"Hello world!";

    loop {
        let confirm = channel_a
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload,
                BasicProperties::default(),
            )
            .await?
            .await?;
        assert_eq!(confirm, Confirmation::NotRequested);
    }
}

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let runtime = Runtime::async_global_executor();

    try_rabbit_stuff(addr, runtime.clone());

    runtime.block_on(std::future::pending())
}
