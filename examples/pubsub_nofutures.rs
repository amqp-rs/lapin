use lapin::{
    message::DeliveryResult, options::*, publisher_confirm::Confirmation, types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ConsumerDelegate,
};
use log::info;
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Debug)]
struct Subscriber {
    channel: Arc<Channel>,
}

impl ConsumerDelegate for Subscriber {
    fn on_new_delivery(
        &self,
        delivery: DeliveryResult,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let channel = self.channel.clone();
        Box::pin(async move {
            if let Ok(Some(delivery)) = delivery {
                channel
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await
                    .expect("basic_ack");
            }
        })
    }
}

fn main() {
    std::env::set_var("RUST_LOG", "info");

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default())
        .wait()
        .expect("connection error");

    info!("CONNECTED");

    let channel_a = conn.create_channel().wait().expect("create_channel");
    let channel_b = conn
        .create_channel()
        .wait()
        .expect("create_channel")
        .into_inner();

    let queue = channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("queue_declare");

    info!("Declared queue {:?}", queue);

    info!("will consume");
    let channel_b = Arc::new(channel_b);
    channel_b
        .clone()
        .basic_consume(
            "hello",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("basic_consume")
        .set_delegate(Subscriber { channel: channel_b });

    let payload = b"Hello world!";

    loop {
        let confirm = channel_a
            .basic_publish(
                "",
                "hello",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .wait()
            .expect("basic_publish")
            .wait()
            .expect("publisher-confirms");
        assert_eq!(confirm, Confirmation::NotRequested);
    }
}
