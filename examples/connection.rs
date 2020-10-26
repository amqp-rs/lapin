use lapin::{
    message::DeliveryResult, options::*, publisher_confirm::Confirmation, types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use log::info;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    async_global_executor::block_on(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .expect("connection error");

        info!("CONNECTED");

        {
            //send channel
            let channel_a = conn.create_channel().await.expect("create_channel");
            //receive channel
            let channel_b = conn.create_channel().await.expect("create_channel");
            info!("[{}] state: {:?}", line!(), conn.status().state());

            //create the hello queue
            let queue = channel_a
                .queue_declare(
                    "hello",
                    QueueDeclareOptions::default(),
                    FieldTable::default(),
                )
                .await
                .expect("queue_declare");
            info!("[{}] state: {:?}", line!(), conn.status().state());
            info!("[{}] declared queue: {:?}", line!(), queue);

            info!("will consume");
            channel_b
                .basic_consume(
                    "hello",
                    "my_consumer",
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await
                .expect("basic_consume")
                .set_delegate(move |delivery: DeliveryResult| async move {
                    info!("received message: {:?}", delivery);
                    if let Ok(Some((channel, delivery))) = delivery {
                        channel
                            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                            .await
                            .expect("basic_ack");
                        channel
                            .basic_cancel("my_consumer", BasicCancelOptions::default())
                            .await
                            .expect("basic_cancel");
                    }
                })
                .expect("set_delegate");
            info!("[{}] state: {:?}", line!(), conn.status().state());

            info!("will publish");
            let payload = b"Hello world!";
            let confirm = channel_a
                .basic_publish(
                    "",
                    "hello",
                    BasicPublishOptions::default(),
                    payload.to_vec(),
                    BasicProperties::default(),
                )
                .await
                .expect("basic_publish")
                .await
                .expect("publisher-confirms");
            assert_eq!(confirm, Confirmation::NotRequested);
            info!("[{}] state: {:?}", line!(), conn.status().state());
        }

        conn.run().expect("conn.run");
    });
}
