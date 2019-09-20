use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties};
use log::info;

fn main() {
    std::env::set_var("RUST_LOG", "trace");

    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default())
        .wait()
        .expect("connection error");

    info!("CONNECTED");

    //now connected

    //receive channel
    let channel = conn.create_channel().wait().expect("create_channel");
    info!("[{}] state: {:?}", line!(), conn.status().state());

    let queue = channel
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("queue_declare");
    info!("[{}] state: {:?}", line!(), conn.status().state());

    info!("will consume");
    let consumer = channel
        .basic_consume(
            &queue,
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("basic_consume");
    info!("[{}] state: {:?}", line!(), conn.status().state());

    for delivery in consumer {
        info!("received message: {:?}", delivery);
        if let Ok(delivery) = delivery {
            channel
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .wait()
                .expect("basic_ack");
        }
    }
}
