use lapin::{
    message::DeliveryResult,
    options::*,
    publisher_confirm::Confirmation,
    tcp::{AMQPUriTcpExt, NativeTlsConnector},
    types::FieldTable,
    uri::AMQPUri,
    BasicProperties, Connection, ConnectionProperties, ConsumerDelegate, Result,
};
use std::{future::Future, pin::Pin};
use tracing::info;

#[derive(Clone, Debug, PartialEq)]
struct Subscriber;

impl ConsumerDelegate for Subscriber {
    fn on_new_delivery(
        &self,
        delivery: DeliveryResult,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            info!(message=?delivery, "received message");
        })
    }
}

async fn connect() -> Result<Connection> {
    // You need to use amqp:// scheme here to handle the TLS part manually as it's automatic when you use amqps://
    let uri = std::env::var("AMQP_ADDR")
        .unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into())
        .parse::<AMQPUri>()
        .unwrap();
    let connect = move |uri: &AMQPUri| {
        uri.connect().and_then(|stream| {
            let tls_builder = NativeTlsConnector::builder();
            // Perform here your custom TLS setup, with tls_builder.identity or whatever else you need
            stream.into_native_tls(
                &tls_builder.build().expect("TLS configuration failed"),
                &uri.authority.host,
            )
        })
    };
    Connection::connector(uri, Box::new(connect), ConnectionProperties::default()).await
}

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    async_global_executor::block_on(async {
        let conn = connect().await.expect("connection error");

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
            .set_delegate(Subscriber);
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
            .await
            .expect("publisher-confirms");
        assert_eq!(confirm, Confirmation::NotRequested);
        info!(state=?conn.status().state());
    })
}
