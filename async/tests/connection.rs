use env_logger;
use lapin_async as lapin;

use std::{net::TcpStream, thread, time};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use crate::lapin::buffer::Buffer;
use crate::lapin::channel::BasicProperties;
use crate::lapin::channel::options::*;
use crate::lapin::connection::*;
use crate::lapin::consumer::ConsumerSubscriber;
use crate::lapin::message::Delivery;
use crate::lapin::types::*;

#[derive(Debug)]
struct Subscriber {
    hello_world: Arc<AtomicBool>,
}

impl ConsumerSubscriber for Subscriber {
    fn new_delivery(&self, delivery: Delivery) {
      println!("received message: {:?}", delivery);
      println!("data: {}", std::str::from_utf8(&delivery.data).unwrap());

      assert_eq!(delivery.data, b"Hello world!");

      self.hello_world.store(true, Ordering::Relaxed);
    }
    fn drop_prefetched_messages(&self) {}
    fn cancel(&self) {}
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
      conn.configuration.set_frame_max(capacity);
      assert_eq!(conn.connect(Credentials::default(), ConnectionProperties::default()).unwrap(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader(Credentials::default(), ConnectionProperties::default())));
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

      let frame_max = conn.configuration.frame_max();
      if frame_max > capacity {
        send_buffer.grow(frame_max as usize);
        receive_buffer.grow(frame_max as usize);
      }

      let channel_a = conn.channels.create().unwrap();
      let channel_b = conn.channels.create().unwrap();
      //send channel
      channel_a.channel_open().expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //receive channel
      channel_b.channel_open().expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //create the hello queue
      channel_a.queue_declare("hello-async", QueueDeclareOptions::default(), FieldTable::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      //purge the hello queue in case it already exists with contents in it
      channel_a.queue_purge("hello-async", QueuePurgeOptions::default()).expect("queue_purge");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      channel_b.queue_declare("hello-async", QueueDeclareOptions::default(), FieldTable::new()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      println!("will consume");
      let hello_world = Arc::new(AtomicBool::new(false));
      let subscriber = Subscriber { hello_world: hello_world.clone(), };
      channel_b.basic_consume("hello-async", "my_consumer", BasicConsumeOptions::default(), FieldTable::new(), Box::new(subscriber)).expect("basic_consume");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());

      println!("will publish");
      let payload = b"Hello world!";
      channel_a.basic_publish("", "hello-async", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).expect("basic_publish");
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      println!("[{}] state: {:?}", line!(), conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap());
      thread::sleep(time::Duration::from_millis(100));
      assert!(hello_world.load(Ordering::Relaxed));
}
