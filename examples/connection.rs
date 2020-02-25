use lapin::{
    message::DeliveryResult, options::*, types::FieldTable, BasicProperties, Connection,
    ConnectionProperties,
};
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

    //send channel
    let channel_a = conn.create_channel().wait().expect("create_channel");
    //receive channel
    let channel_b = conn.create_channel().wait().expect("create_channel");
    info!("[{}] state: {:?}", line!(), conn.status().state());

    //create the hello queue
    let queue = channel_a
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("queue_declare");
    info!("[{}] state: {:?}", line!(), conn.status().state());
    info!("[{}] declared queue: {:?}", line!(), queue);

    let queue = channel_a
        .confirm_select(ConfirmSelectOptions::default())
        .wait()
        .expect("confirm_select");
    info!("[{}] state: {:?}", line!(), conn.status().state());
    info!("Declared queue {:?}", queue);

    let chan = channel_b.clone();
    info!("will consume");
    channel_b
        .basic_consume(
            "hello",
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("basic_consume")
        .set_delegate(Box::new(move |delivery: DeliveryResult| {
            info!("received message: {:?}", delivery);
            if let Ok(Some(delivery)) = delivery {
                chan.basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .wait()
                    .expect("basic_ack");
            }
        }));
    info!("[{}] state: {:?}", line!(), conn.status().state());

    info!("will publish");
    let payload = b"Hello world!";
    channel_a
        .basic_publish(
            "",
            "hello",
            BasicPublishOptions::default(),
            payload.to_vec(),
            BasicProperties::default(),
        )
        .wait()
        .expect("basic_publish");
    info!("[{}] state: {:?}", line!(), conn.status().state());

    std::thread::sleep(std::time::Duration::from_millis(2000));
    conn.close(200, "OK").wait().expect("connection close");
}
