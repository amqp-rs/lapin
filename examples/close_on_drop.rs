use futures_executor::LocalPool;
use lapin::{
    CloseOnDrop,
    message::DeliveryResult, options::*, publisher_confirm::Confirmation, types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use log::info;

fn main() {
    std::env::set_var("RUST_LOG", "trace");

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let mut executor = LocalPool::new();

    executor.run_until(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .map(CloseOnDrop::new)
            .expect("connection error");

        info!("CONNECTED");

        info!("creating then auto-closing channel");
        let _ = conn.create_channel().await.map(CloseOnDrop::new).expect("create_channel");

        let channel = conn.create_channel().await.expect("create_channel");

        channel
            .queue_declare(
                "hello",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("queue_declare");

        let chan = channel.clone();
        channel
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("basic_consume")
            .set_delegate(Box::new(move |delivery: DeliveryResult| {
                info!("received message: {:?}", delivery);
                if let Ok(Some(delivery)) = delivery {
                    chan.basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                        .wait() // await is hard to handle here
                        .expect("basic_ack");
                }
            }));

        let payload = b"Hello world!";
        let confirm = channel
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .await
            .expect("basic_publish")
            .await;
        assert_eq!(confirm, Confirmation::NotRequested);

        std::thread::sleep(std::time::Duration::from_millis(2000));

        info!("Connection will be automatically closed on drop");
    })
}
