use futures_util::stream::StreamExt;
use lapin::{options::*, topology::*, BasicProperties, Connection, ConnectionProperties, Result};
use log::info;

fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let topology_str = std::fs::read_to_string("examples/topology.json").unwrap();
    let topology = serde_json::from_str::<TopologyDefinition>(&topology_str).unwrap();

    async_global_executor::block_on(async {
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default().with_default_executor(8),
        )
        .await?;

        info!("CONNECTED");

        let mut restored = conn.restore(topology).await?;
        let trash_queue = restored.queues.pop().unwrap();
        let channel_b = restored.channels.pop().unwrap().channel;
        let mut c = restored.channels.pop().unwrap();
        let tmp_queue = c.queues.pop().unwrap();
        let mut consumer = c.consumers.pop().unwrap();
        let channel_a = c.channel;

        info!("Declared queues {:?} and {:?}", trash_queue, tmp_queue);

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
        assert_eq!(b"test payload".to_vec(), delivery.unwrap()?.1.data);

        channel_a
            .basic_cancel(consumer.tag().as_str(), BasicCancelOptions::default())
            .await?;

        Ok(())
    })
}
