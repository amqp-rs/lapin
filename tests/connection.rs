use lapin::{
    message::DeliveryResult, options::*, publisher_confirm::Confirmation, types::FieldTable,
    BasicProperties, Connection, ConnectionProperties, ConsumerDelegate,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    thread, time,
};
use tracing::info;

#[derive(Clone, Debug)]
struct Subscriber {
    hello_world: Arc<AtomicU8>,
}

impl ConsumerDelegate for Subscriber {
    fn on_new_delivery(
        &self,
        delivery: DeliveryResult,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let subscriber = self.clone();
        Box::pin(async move {
            info!(message=?delivery, "received message");

            if let Some(delivery) = delivery.unwrap() {
                info!(data=%std::str::from_utf8(&delivery.data).unwrap());

                assert_eq!(delivery.data, b"Hello world!");

                subscriber.hello_world.fetch_add(1, Ordering::SeqCst);

                delivery.ack(BasicAckOptions::default()).await.unwrap();
            }
        })
    }
}

#[test]
fn connection() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    let _ = tracing_subscriber::fmt::try_init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    async_global_executor::block_on(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .expect("connection error");

        info!(configuration=?conn.configuration(), "CONNECTED");

        //now connected

        //send channel
        let channel_a = conn.create_channel().await.expect("create_channel");
        //receive channel
        let channel_b = conn.create_channel().await.expect("create_channel");

        channel_a
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .expect("confirm_select");

        //create the hello queue
        let queue = channel_a
            .queue_declare(
                "hello-async",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("queue_declare");
        info!(state=?conn.status().state());
        info!(?queue, "Declared queue");

        //purge the hello queue in case it already exists with contents in it
        let queue = channel_a
            .queue_purge("hello-async", QueuePurgeOptions::default())
            .await
            .expect("queue_purge");
        info!(state=?conn.status().state());
        info!(purge=?queue, "Purged queue");

        info!("will consume");
        let hello_world = Arc::new(AtomicU8::new(0));
        let subscriber = Subscriber {
            hello_world: hello_world.clone(),
        };
        let consumer = channel_b
            .basic_consume(
                "hello-async",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("basic_consume");

        info!("will publish");
        let payload = b"Hello world!";
        let confirm = channel_a
            .basic_publish(
                "",
                "hello-async",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .await
            .expect("basic_publish")
            .await
            .expect("publisher-confirms");
        assert_eq!(confirm, Confirmation::Ack(None));

        consumer.set_delegate(subscriber);
        info!(state=?conn.status().state());

        info!("will publish");
        let confirm = channel_a
            .basic_publish(
                "",
                "hello-async",
                BasicPublishOptions::default(),
                payload.to_vec(),
                BasicProperties::default(),
            )
            .await
            .expect("basic_publish")
            .await
            .expect("publisher-confirms");
        assert_eq!(confirm, Confirmation::Ack(None));
        info!(state=?conn.status().state());

        thread::sleep(time::Duration::from_millis(500));
        assert_eq!(hello_world.load(Ordering::SeqCst), 2);
    });
}
