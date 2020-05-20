use futures_executor::LocalPool;
use lapin::{
    message::{BasicReturnMessage, Delivery, DeliveryResult},
    options::*,
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use log::info;
use std::sync::Arc;

fn main() {
    std::env::set_var("RUST_LOG", "info");

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    LocalPool::new().run_until(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .expect("connection error");

        info!("CONNECTED");

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

        channel_a
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .expect("confirm_select");
        info!("[{}] state: {:?}", line!(), conn.status().state());
        info!("Enabled publisher-confirms");

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
            .await // Wait for this specific ack/nack
            .expect("publisher-confirms");
        assert!(confirm.is_ack());
        assert_eq!(confirm.take_message(), None);
        info!("[{}] state: {:?}", line!(), conn.status().state());

        for _ in 1..=2 {
            channel_a
                .basic_publish(
                    "",
                    "hello",
                    BasicPublishOptions::default(),
                    payload.to_vec(),
                    BasicProperties::default(),
                )
                .await
                .expect("basic_publish"); // Drop the PublisherConfirm instead for waiting for it ...
        }

        // ... and wait for all pending ack/nack afterwards instead of individually in the above loop
        let returned = channel_a
            .wait_for_confirms()
            .await
            .expect("wait for confirms");
        assert!(returned.is_empty());

        let confirm = channel_a
            .basic_publish(
                "",
                "unroutable-routing-key-for-tests",
                BasicPublishOptions {
                    mandatory: true,
                    ..BasicPublishOptions::default()
                },
                payload.to_vec(),
                BasicProperties::default().with_priority(42),
            )
            .await
            .expect("basic_publish")
            .await // Wait for this specific ack/nack
            .expect("publisher-confirms");
        assert!(confirm.is_ack());
        assert_eq!(
            confirm.take_message(),
            Some(BasicReturnMessage {
                delivery: Delivery {
                    delivery_tag: 0,
                    exchange: "".into(),
                    routing_key: "unroutable-routing-key-for-tests".into(),
                    redelivered: false,
                    properties: BasicProperties::default().with_priority(42),
                    data: payload.to_vec(),
                },
                reply_code: 312,
                reply_text: "NO_ROUTE".into(),
            })
        );

        let _ = channel_a;
    })
}
