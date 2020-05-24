use futures_executor::LocalPool;
use lapin::{
    message::DeliveryResult,
    options::*,
    publisher_confirm::Confirmation,
    tcp::{Identity, TLSConfig},
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use log::info;
use std::sync::Arc;

fn get_tls_config() -> TLSConfig<'static, 'static, 'static> {
    let cert_chain = "" /* include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/path/to/ca_certificate.pem"
    )) */;
    let client_cert_and_key = b""
        /* include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/path/to/client.pfx")) */;
    let client_cert_and_key_password = "bunnies";

    TLSConfig {
        identity: Some(Identity {
            der: client_cert_and_key,
            password: client_cert_and_key_password,
        }),
        cert_chain: Some(cert_chain),
    }
}

fn main() {
    env_logger::init();

    let addr = std::env::var("AMQP_ADDR")
        .unwrap_or_else(|_| "amqps://localhost:5671/%2f?auth_mechanism=external".into());

    LocalPool::new().run_until(async {
        let conn = Connection::connect_with_config(
            &addr,
            ConnectionProperties::default(),
            get_tls_config(),
        )
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
            .await
            .expect("publisher-confirms");
        assert_eq!(confirm, Confirmation::NotRequested);
        info!("[{}] state: {:?}", line!(), conn.status().state());
    })
}
