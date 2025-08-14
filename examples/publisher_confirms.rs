use lapin::{
    BasicProperties, Connection, ConnectionProperties,
    message::{BasicReturnMessage, Delivery, DeliveryResult},
    options::*,
    protocol::{AMQPErrorKind, AMQPSoftError},
    types::FieldTable,
};
use tracing::info;

async fn tokio_main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    let conn = Connection::connect(&addr, ConnectionProperties::default())
        .await
        .expect("connection error");

    info!("CONNECTED");

    //send channel
    let channel_a = conn.create_channel().await.expect("create_channel");
    //receive channel
    let channel_b = conn.create_channel().await.expect("create_channel");
    info!(state=?conn.status().state());

    //create the hello queue
    let queue = channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .expect("queue_declare");
    info!(state=?conn.status().state());
    info!(?queue, "Declared queue");

    channel_a
        .confirm_select(ConfirmSelectOptions::default())
        .await
        .expect("confirm_select");
    info!(state=?conn.status().state());
    info!("Enabled publisher-confirms");

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
            info!(message=?delivery, "received message");
            if let Ok(Some(delivery)) = delivery {
                delivery
                    .ack(BasicAckOptions::default())
                    .await
                    .expect("basic_ack");
            }
        });
    info!(state=?conn.status().state());

    info!("will publish");
    let payload = b"Hello world!";
    let confirm = channel_a
        .basic_publish(
            "",
            "hello",
            BasicPublishOptions::default(),
            payload,
            BasicProperties::default(),
        )
        .await
        .expect("basic_publish")
        .await // Wait for this specific ack/nack
        .expect("publisher-confirms");
    assert!(confirm.is_ack());
    assert_eq!(confirm.take_message(), None);
    info!(state=?conn.status().state());

    for _ in 1..=2 {
        channel_a
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload,
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
            payload,
            BasicProperties::default().with_priority(42),
        )
        .await
        .expect("basic_publish")
        .await // Wait for this specific ack/nack
        .expect("publisher-confirms");
    assert!(confirm.is_ack());
    let message = confirm.take_message().unwrap();
    let acker = message.delivery.acker.clone();
    assert_eq!(
        message,
        BasicReturnMessage {
            delivery: Delivery {
                delivery_tag: 0,
                exchange: "".into(),
                routing_key: "unroutable-routing-key-for-tests".into(),
                redelivered: false,
                properties: BasicProperties::default().with_priority(42),
                data: payload.to_vec(),
                acker,
            },
            reply_code: 312,
            reply_text: "NO_ROUTE".into(),
        }
    );
    let error = message.error().unwrap();
    assert_eq!(error.kind(), &AMQPErrorKind::Soft(AMQPSoftError::NOROUTE));

    let _ = channel_a;
}

#[tokio::main]
async fn main() {
    tokio_main().await
}

#[tokio::test]
async fn connection() {
    tokio_main().await
}
