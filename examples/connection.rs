use futures_executor::LocalPool;
use lapin::{
    message::DeliveryResult, options::*, publisher_confirm::Confirmation, types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use log::info;
use std::sync::Arc;

fn main() {
    std::env::set_var("RUST_LOG", "trace");

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    LocalPool::new().run_until(async {
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

            let channel_b = Arc::new(channel_b);
            let chan = channel_b.clone();
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
                .set_delegate(move |delivery: DeliveryResult| {
                    let chan = chan.clone();
                    async move {
                        info!("received message: {:?}", delivery);
                        if let Ok(Some(delivery)) = delivery {
                            chan.basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                                .await
                                .expect("basic_ack");
                            chan.basic_cancel("my_consumer", BasicCancelOptions::default())
                                .await
                                .expect("basic_cancel");
                        }
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
