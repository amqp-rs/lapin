# lapin-async

this library is meant for use in an event loop. The library exposes, through the
[Connection struct](https://docs.rs/lapin-async/0.1.0/lapin_async/connection/struct.Connection.html),
a state machine you can drive through IO you manage.

Typically, your code would own the socket and buffers, and regularly pass the
input and output buffers to the state machine so it receives messages and
serializes new ones to send. You can then query the current state and see
if it received new messages for the consumers.

## Example

```rust
use env_logger;
use lapin_async as lapin;

use std::{
  net::TcpStream,
  thread,
  time,
};

use crate::lapin::{
  channel::BasicProperties,
  channel_status::ChannelState,
  channel::options::*,
  connection::Connection,
  connection_properties::ConnectionProperties,
  connection_status::{ConnectionState, ConnectingState},
  consumer::ConsumerSubscriber,
  credentials::Credentials,
  io_loop::IoLoop,
  message::Delivery,
  types::FieldTable,
};

fn main() {
  env_logger::init();

  /* Open TCP connection */
  let stream = TcpStream::connect("127.0.0.1:5672").unwrap();
  stream.set_nonblocking(true).unwrap();

  /* Configure AMQP connection */
  let capacity = 8192;
  let conn: Connection = Connection::default();
  conn.configuration.set_frame_max(capacity);

  /* Connect tp RabbitMQ server */
  conn.connect(Credentials::default(), ConnectionProperties::default()).expect("connect");
  assert_eq!(conn.status.state(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader(Credentials::default(), ConnectionProperties::default())));
  IoLoop::new(conn.clone(), mio::net::TcpStream::from_stream(stream).expect("tcp stream")).expect("io loop").run().expect("io loop");
  loop {
    match conn.status.state() {
      ConnectionState::Connected => break,
      state => println!("now at state {:?}, continue", state),
    }
    thread::sleep(time::Duration::from_millis(100));
  }
  println!("CONNECTED");

  /* Create and open a channel */
  let channel = conn.create_channel().unwrap();
  let request_id = channel.channel_open().expect("channel_open");
  assert!(channel.wait_for_reply(request_id).unwrap_or(false));
  assert!(channel.status.state() == ChannelState::Connected);

  /* Declaire the "hellp" queue */
  let request_id = channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).unwrap();
  assert!(channel.wait_for_reply(request_id).unwrap_or(false));

  /* Publish "Hellow world!" to the "hello" queue */
  let payload = b"Hello world!";
  let request_id = channel.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).expect("basic_publish");
  assert!(channel.wait_for_reply(request_id).unwrap_or(false));

  /* Consumer the messages from the "hello" queue using an instance of Subscriber */
  let request_id = channel.basic_consume("hello", "my_consumer", BasicConsumeOptions::default(), FieldTable::default(), Box::new(Subscriber)).expect("basic_consume");
  assert!(channel.wait_for_reply(request_id).unwrap_or(false));
}

#[derive(Debug)]
struct Subscriber;

impl ConsumerSubscriber for Subscriber {
  fn new_delivery(&self, _delivery: Delivery) {
    // handle message
  }
  fn drop_prefetched_messages(&self) {}
  fn cancel(&self) {}
}
```
