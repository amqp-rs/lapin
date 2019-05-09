use env_logger;
use lapin_async as lapin;
use log::info;

use std::{
  net::TcpStream,
  thread,
  time,
};

use crate::lapin::{
  buffer::Buffer,
  channel::BasicProperties,
  channel::options::*,
  connection::Connection,
  connection_properties::ConnectionProperties,
  connection_status::{ConnectionState, ConnectingState},
  consumer::ConsumerSubscriber,
  credentials::Credentials,
  message::Delivery,
  types::FieldTable,
};

#[derive(Clone,Debug,PartialEq)]
struct Subscriber;

impl ConsumerSubscriber for Subscriber {
    fn new_delivery(&self, delivery: Delivery) {
      info!("received message: {:?}", delivery);
    }
    fn drop_prefetched_messages(&self) {}
    fn cancel(&self) {}
}

fn main() {
      std::env::set_var("RUST_LOG", "trace");

      env_logger::init();

      let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "127.0.0.1:5672".to_string());
      let mut stream = TcpStream::connect(&addr).unwrap();
      stream.set_nonblocking(true).unwrap();

      let capacity = 8192;
      let mut send_buffer    = Buffer::with_capacity(capacity as usize);
      let mut receive_buffer = Buffer::with_capacity(capacity as usize);

      let mut conn: Connection = Connection::new();
      conn.configuration.set_frame_max(capacity);
      assert_eq!(conn.connect(Credentials::default(), ConnectionProperties::default()).unwrap(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader(Credentials::default(), ConnectionProperties::default())));
      loop {
        match conn.run(&mut stream, &mut send_buffer, &mut receive_buffer) {
          Err(e) => panic!("could not connect: {:?}", e),
          Ok(ConnectionState::Connected) => break,
          Ok(state) => info!("now at state {:?}, continue", state),
        }
        thread::sleep(time::Duration::from_millis(100));
      }
      info!("CONNECTED");

      //now connected

      let frame_max = conn.configuration.frame_max();
      if frame_max > capacity {
        send_buffer.grow(frame_max as usize);
        receive_buffer.grow(frame_max as usize);
      }

      let channel_a = conn.create_channel().unwrap();
      let channel_b = conn.create_channel().unwrap();
      //send channel
      channel_a.channel_open().expect("channel_open");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //receive channel
      channel_b.channel_open().expect("channel_open");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //create the hello queue
      channel_a.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::new()).expect("queue_declare");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      channel_a.confirm_select(ConfirmSelectOptions::default()).expect("confirm_select");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      channel_b.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::new()).expect("queue_declare");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      info!("will consume");
      channel_b.basic_consume("hello", "my_consumer", BasicConsumeOptions::default(), FieldTable::new(), Box::new(Subscriber)).expect("basic_consume");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      info!("will publish");
      let payload = b"Hello world!";
      channel_a.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).expect("basic_publish");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      panic!();
}

