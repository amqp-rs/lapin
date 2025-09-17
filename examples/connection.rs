use async_rs::{Runtime, traits::*};
use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties, message::DeliveryResult,
    options::*, types::FieldTable,
};
use tracing::info;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let runtime = Runtime::async_global_executor();

    runtime.clone().block_on(async move {
        let conn = Connection::connect_with_runtime(
            &addr,
            ConnectionProperties::default(),
            runtime.clone(),
        )
        .await
        .expect("connection error");

        let mut events_listener = conn.events_listener();

        runtime.spawn(async move {
            while let Some(event) = events_listener.next().await {
                info!(?event, "GOT EVENT");
            }
        });

        info!("CONNECTED");

        {
            //send channel
            let channel_a = conn.create_channel().await.expect("create_channel");

            //receive channel
            let channel_b = conn.create_channel().await.expect("create_channel");
            info!(state=?conn.status());

            //create the hello queue
            let queue = channel_a
                .queue_declare(
                    "hello".into(),
                    QueueDeclareOptions::default(),
                    FieldTable::default(),
                )
                .await
                .expect("queue_declare");
            info!(state=?conn.status());
            info!(?queue, "Declared queue");

            info!("will consume");
            let channel = channel_b.clone();
            channel_b
                .basic_consume(
                    "hello".into(),
                    "my_consumer".into(),
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await
                .expect("basic_consume")
                .set_delegate(move |delivery: DeliveryResult| {
                    let channel = channel.clone();
                    async move {
                        info!(message=?delivery, "received message");
                        if let Ok(Some(delivery)) = delivery {
                            delivery
                                .ack(BasicAckOptions::default())
                                .await
                                .expect("basic_ack");
                            channel
                                .basic_cancel("my_consumer".into(), BasicCancelOptions::default())
                                .await
                                .expect("basic_cancel");
                        }
                    }
                });
            info!(state=?conn.status());

            info!("will publish");
            let payload = b"Hello world!";
            let confirm = channel_a
                .basic_publish(
                    "".into(),
                    "hello".into(),
                    BasicPublishOptions::default(),
                    payload,
                    BasicProperties::default(),
                )
                .await
                .expect("basic_publish")
                .await
                .expect("publisher-confirms");
            assert_eq!(confirm, Confirmation::NotRequested);
            info!(state=?conn.status());
        }

        conn.run().expect("conn.run");
    });
}
