use futures_util::stream::StreamExt;
use lapin::{
    options::*,
    types::{AMQPValue, FieldTable},
    Connection, ConnectionProperties, ExchangeKind, Result,
};
use log::info;

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    async_global_executor::block_on(async {
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

        channel_a
            .exchange_declare(
                "test-exchange",
                ExchangeKind::Direct,
                ExchangeDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;
        channel_a
            .queue_bind(
                queue.name().as_str(),
                "test-exchange",
                "test-rk",
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let mut dloptions = FieldTable::default();
        dloptions.insert("x-message-ttl".into(), AMQPValue::LongUInt(2000));
        dloptions.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString("test-exchange".into()),
        );
        dloptions.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString("test-rk".into()),
        );
        channel_a
            .queue_declare("trash-queue", QueueDeclareOptions::default(), dloptions)
            .await?;

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
                let (_, delivery) = delivery.expect("error in consumer");
                delivery.ack(BasicAckOptions::default()).await.expect("ack");
            }
        })
        .detach();

        println!(
            "Topology: {}",
            serde_json::to_string_pretty(&conn.topology()).unwrap()
        );

        Ok(())
    })
}
