use env_logger;
use lapin_async as lapin;
use crate::lapin::buffer::Buffer;
use crate::lapin::channel::BasicProperties;
use crate::lapin::connection::*;
use crate::lapin::consumer::ConsumerSubscriber;
use crate::lapin::message::Delivery;
use crate::lapin::types::*;
use log::info;

use std::{net::TcpStream, thread, time};

#[derive(Clone,Debug,PartialEq)]
struct Subscriber;

impl ConsumerSubscriber for Subscriber {
    fn new_delivery(&mut self, delivery: Delivery) {
      info!("received message: {:?}", delivery);
    }
    fn drop_prefetched_messages(&mut self) {}
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
      conn.set_frame_max(capacity);
      assert_eq!(conn.connect().unwrap(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader));
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

      let frame_max = conn.configuration.frame_max;
      if frame_max > capacity {
        send_buffer.grow(frame_max as usize);
        receive_buffer.grow(frame_max as usize);
      }

      let channel_a: u16 = conn.create_channel().unwrap();
      let channel_b: u16 = conn.create_channel().unwrap();
      //send channel
      conn.channel_open(channel_a, "".to_string()).expect("channel_open");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //receive channel
      conn.channel_open(channel_b, "".to_string()).expect("channel_open");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //create the hello queue
      conn.queue_declare(channel_a, 0, "hello".to_string(), false, false, false, false, false, FieldTable::new()).expect("queue_declare");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      conn.confirm_select(channel_a, false).expect("confirm_select");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      conn.queue_declare(channel_b, 0, "hello".to_string(), false, false, false, false, false, FieldTable::new()).expect("queue_declare");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      info!("will consume");
      conn.basic_consume(channel_b, 0, "hello".to_string(), "my_consumer".to_string(), false, true, false, false, FieldTable::new(), Box::new(Subscriber)).expect("basic_consume");
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      info!("will publish");
      conn.basic_publish(channel_a, 0, "".to_string(), "hello".to_string(), false, false).expect("basic_publish");
      let payload = b"Hello world!";
      conn.send_content_frames(channel_a, 60, payload, BasicProperties::default());
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      info!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      panic!();
}

