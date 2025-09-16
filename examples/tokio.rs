use async_rs::Runtime;
use lapin::{
    BasicProperties, Connection, ConnectionProperties,
    message::DeliveryResult,
    options::{
        BasicAckOptions, BasicCancelOptions, BasicConsumeOptions, BasicPublishOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
};

async fn tokio_main(forever: bool) {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let connection = Connection::connect_with_runtime(
        &addr,
        ConnectionProperties::default(),
        Runtime::tokio_current(),
    )
    .await
    .unwrap();
    let channel = connection.create_channel().await.unwrap();

    let _queue = channel
        .queue_declare(
            "queue_test".into(),
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let consumer = channel
        .basic_consume(
            "queue_test".into(),
            "tag_foo".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    consumer.set_delegate(move |delivery: DeliveryResult| async move {
        let delivery = match delivery {
            // Carries the delivery alongside its channel
            Ok(Some(delivery)) => delivery,
            // The consumer got canceled
            Ok(None) => return,
            // Carries the error and is always followed by Ok(None)
            Err(error) => {
                dbg!("Failed to consume queue message {}", error);
                return;
            }
        };

        // Do something with the delivery data (The message payload)

        delivery
            .ack(BasicAckOptions::default())
            .await
            .expect("Failed to ack send_webhook_event message");
    });

    channel
        .basic_publish(
            "".into(),
            "queue_test".into(),
            BasicPublishOptions::default(),
            b"Hello world!",
            BasicProperties::default(),
        )
        .await
        .unwrap()
        .await
        .unwrap();

    if forever {
        std::future::pending::<()>().await;
    } else {
        channel
            .basic_cancel("tag_foo".into(), BasicCancelOptions::default())
            .await
            .unwrap();
        connection.close(200, "OK".into()).await.unwrap();
    }
}

#[tokio::main]
async fn main() {
    tokio_main(true).await
}

#[tokio::test]
async fn connection() {
    tokio_main(false).await
}
