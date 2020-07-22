use futures_executor::{LocalPool, ThreadPool};
use futures_util::stream::StreamExt;
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use log::info;
use std::sync::Arc;

fn retry_rabbit_stuff(addr: String, executor: Arc<ThreadPool>) {
    std::thread::sleep(std::time::Duration::from_millis(2000));
    log::debug!("Reconnecting to rabbitmq");
    try_rabbit_stuff(addr, executor);
}

fn try_rabbit_stuff(addr: String, executor: Arc<ThreadPool>) {
    executor.clone().spawn_ok(async move {
        if let Err(err) = rabbit_stuff(addr.clone(), executor.clone()).await {
            log::error!("Error: {}", err);
            retry_rabbit_stuff(addr, executor);
        }
    });
}

async fn rabbit_stuff(addr: String, executor: Arc<ThreadPool>) -> Result<()> {
    let conn = Connection::connect(
        &addr,
        ConnectionProperties::default().with_default_executor(8),
    )
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

    info!("Declared queue {:?}", queue);

    let mut consumer = channel_b
        .basic_consume(
            "hello",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    executor.spawn_ok(async move {
        info!("will consume");
        while let Some(delivery) = consumer.next().await {
            let (channel, delivery) = delivery.expect("error in consumer");
            channel
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .await
                .expect("ack");
        }
    });

    let payload = b"Hello world!";

    loop {
        let confirm = channel_a
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .await?
            .await?;
        assert_eq!(confirm, Confirmation::NotRequested);
    }
}

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let executor = Arc::new(ThreadPool::new()?);

    try_rabbit_stuff(addr, executor);

    LocalPool::new().run_until(futures_util::future::pending())
}
