use futures_lite::stream::StreamExt;
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use log::info;

fn retry_rabbit_stuff(addr: String) {
    std::thread::sleep(std::time::Duration::from_millis(2000));
    log::debug!("Reconnecting to rabbitmq");
    try_rabbit_stuff(addr);
}

fn try_rabbit_stuff(addr: String) {
    async_global_executor::spawn(async move {
        if let Err(err) = rabbit_stuff(addr.clone()).await {
            log::error!("Error: {}", err);
            retry_rabbit_stuff(addr);
        }
    })
    .detach();
}

async fn rabbit_stuff(addr: String) -> Result<()> {
    let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;

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
    async_global_executor::spawn(async move {
        info!("will consume");
        while let Some(delivery) = consumer.next().await {
            let (channel, delivery) = delivery.expect("error in consumer");
            channel
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .await
                .expect("ack");
        }
    })
    .detach();

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

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    try_rabbit_stuff(addr);

    async_global_executor::block_on(futures_lite::future::pending())
}
