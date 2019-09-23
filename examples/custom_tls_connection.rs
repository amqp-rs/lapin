use amq_protocol::tcp::AMQPUriTcpExt;
use lapin::{
    confirmation::Confirmation, message::Delivery, options::*, types::FieldTable, BasicProperties,
    Connection, ConnectionProperties, ConsumerDelegate, Error, Result,
};
use log::info;
use tcp_stream::{HandshakeError, NativeTlsConnector};

#[derive(Clone, Debug, PartialEq)]
struct Subscriber;

impl ConsumerDelegate for Subscriber {
    fn on_new_delivery(&self, delivery: Result<Option<Delivery>>) {
        info!("received message: {:?}", delivery);
    }
}

fn connect() -> Result<Connection> {
    // You need to use amqp:// scheme here to handle the tls part manually as it's automatic when you use amqps
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let conn = addr
        .connect(|stream, uri, poll| {
            let tls_builder = NativeTlsConnector::builder();
            // Perform here your custom tls setup, with tls_builder.identity or whatever else you need
            let mut res = stream.into_native_tls(
                tls_builder.build().expect("TLS configuration failed"),
                &uri.authority.host,
            );
            while let Err(error) = res {
                match error {
                    HandshakeError::Failure(io_err) => {
                        panic!("TLS connection failed: {:?}", io_err)
                    }
                    HandshakeError::WouldBlock(mid) => res = mid.handshake(),
                }
            }
            let stream = res.unwrap();
            Connection::connector(ConnectionProperties::default())(stream, uri, poll)
        })
        .map_err(Error::IOError)?;
    Confirmation::from(conn).wait()
}

fn main() {
    std::env::set_var("RUST_LOG", "trace");

    env_logger::init();

    let conn = connect().expect("connection error");

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

    channel_a
        .confirm_select(ConfirmSelectOptions::default())
        .wait()
        .expect("confirm_select");
    info!("[{}] state: {:?}", line!(), conn.status().state());

    let queue = channel_b
        .queue_declare(
            "hello",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("queue_declare");
    info!("[{}] state: {:?}", line!(), conn.status().state());

    info!("will consume");
    channel_b
        .basic_consume(
            &queue,
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .wait()
        .expect("basic_consume")
        .set_delegate(Box::new(Subscriber));
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
}
