use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties,
    message::DeliveryResult,
    options::*,
    tcp::{OwnedIdentity, OwnedTLSConfig},
    types::FieldTable,
};
use tracing::info;

fn get_tls_config() -> OwnedTLSConfig {
    let cert_chain = "" /* include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/path/to/ca_certificate.pem"
    )) */;
    let client_cert_and_key = b""
        /* include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/path/to/client.pfx")) */;
    let client_cert_and_key_password = "bunnies";

    OwnedTLSConfig {
        identity: Some(OwnedIdentity::PKCS12 {
            der: client_cert_and_key.to_vec(),
            password: client_cert_and_key_password.to_string(),
        }),
        cert_chain: Some(cert_chain.to_string()),
    }
}

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR")
        .unwrap_or_else(|_| "amqps://localhost:5671/%2f?auth_mechanism=external".into());

    let conn =
        Connection::connect_with_config(&addr, ConnectionProperties::default(), get_tls_config())
            .await
            .expect("connection error");

    info!("CONNECTED");

    //send channel
    let channel_a = conn.create_channel().await.expect("create_channel");
    //receive channel
    let channel_b = conn.create_channel().await.expect("create_channel");
    info!(state=?conn.status());

    //create the hello queue
    let queue = channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await
        .expect("queue_declare");
    info!(state=?conn.status());
    info!(?queue, "Declared queue");

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
    info!(state=?conn.status());

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
        .await
        .expect("publisher-confirms");
    assert_eq!(confirm, Confirmation::NotRequested);
    info!(state=?conn.status());
}
