use futures_executor::LocalPool;
use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties};
use log::info;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    LocalPool::new().run_until(async {
        let conn = Connection::connect(&addr, ConnectionProperties::default())
            .await
            .expect("connection error");

        info!("CONNECTED");

        //receive channel
        let channel = conn.create_channel().await.expect("create_channel");
        info!("[{}] state: {:?}", line!(), conn.status().state());

        let queue = channel
            .queue_declare(
                "hello",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("queue_declare");
        info!("[{}] state: {:?}", line!(), conn.status().state());
        info!("declared queue {:?}", queue);

        info!("will consume");
        let consumer = channel
            .basic_consume(
                "hello",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("basic_consume");
        info!("[{}] state: {:?}", line!(), conn.status().state());

        for delivery in consumer {
            info!("received message: {:?}", delivery);
            if let Ok((channel, delivery)) = delivery {
                channel
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await
                    .expect("basic_ack");
            }
        }
    })
}
