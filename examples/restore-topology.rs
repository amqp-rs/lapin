use futures_lite::stream::StreamExt;
use lapin::{options::*, topology::*, BasicProperties, Connection, ConnectionProperties, Result};
use tracing::info;

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let topology_str = std::fs::read_to_string("examples/topology.json").unwrap();
    let topology = serde_json::from_str::<TopologyDefinition>(&topology_str).unwrap();

    async_global_executor::block_on(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;

        info!("CONNECTED");

        let topology = conn.restore(topology).await?;

        let trash_queue = topology.queue(0);
        let channel_a = topology.channel(0); // Can be used as Channel thanks to Deref
        let channel_b = topology.channel(1).into_inner(); // Get the actual inner Channel
        let tmp_queue = channel_a.queue(0);
        let mut consumer = channel_a.consumer(0);

        info!(?trash_queue, ?tmp_queue, "Declared queues");

        channel_b
            .basic_publish(
                "",
                "trash-queue",
                BasicPublishOptions::default(),
                b"test payload".to_vec(),
                BasicProperties::default(),
            )
            .await?
            .await?;

        let delivery = consumer.next().await;
        assert_eq!(b"test payload".to_vec(), delivery.unwrap()?.data);

        channel_a
            .basic_cancel(consumer.tag().as_str(), BasicCancelOptions::default())
            .await?;

        Ok(())
    })
}
