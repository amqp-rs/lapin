extern crate env_logger;
extern crate lapin_async as lapin;

use std::net::TcpStream;
use std::{thread,time};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use lapin::buffer::Buffer;
use lapin::channel::BasicProperties;
use lapin::connection::*;
use lapin::consumer::ConsumerSubscriber;
use lapin::message::Delivery;
use lapin::types::*;

#[derive(Debug)]
struct Subscriber {
    hello_world: Arc<AtomicBool>,
}

impl ConsumerSubscriber for Subscriber {
    fn new_delivery(&mut self, delivery: Delivery) {
      println!("received message: {:?}", delivery);
      println!("data: {}", std::str::from_utf8(&delivery.data).unwrap());

      assert_eq!(delivery.data, b"Hello world!");

      self.hello_world.store(true, Ordering::Relaxed);
    }
}

#[test]
fn connection() {
      let _ = env_logger::try_init();

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
          Ok(state) => println!("now at state {:?}, continue", state),
        }
        thread::sleep(time::Duration::from_millis(100));
      }
      println!("CONNECTED with configuration: {:?}", conn.configuration);

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
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //receive channel
      conn.channel_open(channel_b, "".to_string()).expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //create the hello queue
      conn.queue_declare(channel_a, 0, "hello-async".to_string(), false, false, false, false, false, FieldTable::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //purge the hello queue in case it already exists with contents in it
      conn.queue_purge(channel_a, 0, "hello-async".to_string(), false).expect("queue_purge");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      conn.queue_declare(channel_b, 0, "hello-async".to_string(), false, false, false, false, false, FieldTable::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      println!("will consume");
      let hello_world = Arc::new(AtomicBool::new(false));
      let subscriber = Subscriber { hello_world: hello_world.clone(), };
      conn.basic_consume(channel_b, 0, "hello-async".to_string(), "my_consumer".to_string(), false, true, false, false, FieldTable::new(), Box::new(subscriber)).expect("basic_consume");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      println!("will publish");
      conn.basic_publish(channel_a, 0, "".to_string(), "hello-async".to_string(), false, false).expect("basic_publish");
      let payload = b"Hello world!";
      conn.send_content_frames(channel_a, 60, payload, BasicProperties::default());
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      assert!(hello_world.load(Ordering::Relaxed));
}
