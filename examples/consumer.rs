use futures_lite::StreamExt;
use lapin::{Connection, ConnectionProperties, options::*, types::FieldTable};
use tracing::info;

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    let conn = Connection::connect(&addr, ConnectionProperties::default())
        .await
        .expect("connection error");

    info!("CONNECTED");

    //receive channel
    let channel = conn.create_channel().await.expect("create_channel");
    info!(state=?conn.status());

    let queue = channel
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
    let mut consumer = channel
        .basic_consume(
            "hello".into(),
            "my_consumer".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .expect("basic_consume");
    info!(state=?conn.status());

    while let Some(delivery) = consumer.next().await {
        info!(message=?delivery, "received message");
        if let Ok(delivery) = delivery {
            delivery
                .ack(BasicAckOptions::default())
                .await
                .expect("basic_ack");
        }
    }
}
