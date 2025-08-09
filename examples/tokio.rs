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
    let options = ConnectionProperties::default()
        // Use tokio executor and reactor.
        // At the moment the reactor is only available for unix.
        .with_executor(tokio_executor_trait::Tokio::current())
        .with_reactor(tokio_reactor_trait::Tokio::current());

    let connection = Connection::connect(&addr, options).await.unwrap();
    let channel = connection.create_channel().await.unwrap();

    let _queue = channel
        .queue_declare(
            "queue_test",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();

    let consumer = channel
        .basic_consume(
            "queue_test",
            "tag_foo",
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
            "",
            "queue_test",
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
            .basic_cancel("tag_foo", BasicCancelOptions::default())
            .await
            .unwrap();
        connection.close(200, "OK").await.unwrap();
    }
}

#[tokio::main]
async fn main() {
    tokio_main(true).await
}

#[test]
fn connection() {
    tokio::runtime::Runtime::new()
        .expect("failed to build tokio runtime")
        .block_on(tokio_main(false));
}
