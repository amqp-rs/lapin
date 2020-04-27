use futures_executor::{LocalPool, ThreadPool};
use futures_util::{future::FutureExt, stream::StreamExt};
use lapin::{
    options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties, Result,
};
use log::info;
use std::sync::Arc;

fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "info");

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let executor = ThreadPool::new()?;

    LocalPool::new().run_until(async {
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

        let channel_b = Arc::new(channel_b);
        let consumer = channel_b
            .clone()
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        executor.spawn_ok(async move {
            info!("will consume");
            consumer
                .for_each(move |delivery| {
                    let delivery = delivery.expect("error caught in in consumer");
                    channel_b
                        .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                        .map(|_| ())
                })
                .await
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
    })
}
