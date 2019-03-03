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
use crate::lapin::api::ChannelState;
use crate::lapin::buffer::Buffer;
use crate::lapin::connection::*;
use crate::lapin::consumer::ConsumerSubscriber;
use crate::lapin::channel::BasicProperties;
use crate::lapin::message::Delivery;
use crate::lapin::types::FieldTable;

use std::{net::TcpStream, thread, time};

fn main() {
  env_logger::init();

  /* Open TCP connection */
  let mut stream = TcpStream::connect("127.0.0.1:5672").unwrap();
  stream.set_nonblocking(true).unwrap();

  /* Configure AMQP connection */
  let capacity = 8192;
  let mut send_buffer    = Buffer::with_capacity(capacity as usize);
  let mut receive_buffer = Buffer::with_capacity(capacity as usize);
  let mut conn: Connection = Connection::new();
  conn.set_frame_max(capacity);

  /* Connect tp RabbitMQ server */
  assert_eq!(conn.connect(ConnectionProperties::default()).unwrap(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader(ConnectionProperties::default())));
  loop {
    match conn.run(&mut stream, &mut send_buffer, &mut receive_buffer) {
      Err(e) => panic!("could not connect: {:?}", e),
      Ok(ConnectionState::Connected) => break,
      Ok(state) => println!("now at state {:?}, continue", state),
    }
    thread::sleep(time::Duration::from_millis(100));
  }
  println!("CONNECTED");

  /* Adapt our buffer after negocation with the server */
  let frame_max = conn.configuration.frame_max;
  if frame_max > capacity {
    send_buffer.grow(frame_max as usize);
    receive_buffer.grow(frame_max as usize);
  }

  /* Create and open a channel */
  let channel_id = conn.create_channel().unwrap();
  conn.channel_open(channel_id, "".to_string()).expect("channel_open");
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
  thread::sleep(time::Duration::from_millis(100));
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
  assert!(conn.check_state(channel_id, ChannelState::Connected).map_err(|e| println!("{:?}", e)).is_ok());

  /* Declaire the "hellp" queue */
  let request_id = conn.queue_declare(channel_id, 0, "hello".to_string(), false, false, false, false, false, FieldTable::default()).unwrap();
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
  thread::sleep(time::Duration::from_millis(100));
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
  assert!(conn.is_finished(request_id).unwrap_or(false));

  /* Publish "Hellow world!" to the "hello" queue */
  conn.basic_publish(channel_id, 0, "".to_string(), "hello".to_string(), false, false).expect("basic_publish");
  let payload = b"Hello world!";
  conn.send_content_frames(channel_id, 60, payload, BasicProperties::default());
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
  thread::sleep(time::Duration::from_millis(100));
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();

  /* Consumer the messages from the "hello" queue using an instance of Subscriber */
  let request_id = conn.basic_consume(channel_id, 0, "hello".to_string(), "my_consumer".to_string(), false, true, false, false, FieldTable::default(), Box::new(Subscriber)).expect("basic_consume");
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
  thread::sleep(time::Duration::from_millis(100));
  conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
  assert!(conn.is_finished(request_id).unwrap_or(false));
}

#[derive(Debug)]
struct Subscriber;

impl ConsumerSubscriber for Subscriber {
  fn new_delivery(&mut self, _delivery: Delivery) {
    // handle message
  }
}

```
