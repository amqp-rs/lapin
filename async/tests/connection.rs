use env_logger;
use lapin_async as lapin;

use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time,
};

use crate::lapin::{
  channel::BasicProperties,
  channel::options::*,
  connection::Connection,
  connection_properties::ConnectionProperties,
  connection_status::{ConnectionState, ConnectingState},
  consumer::ConsumerSubscriber,
  credentials::Credentials,
  message::Delivery,
  tcp::AMQPUriTcpExt,
  types::FieldTable,
};

#[derive(Debug)]
struct Subscriber {
    hello_world: Arc<AtomicBool>,
}

impl ConsumerSubscriber for Subscriber {
    fn new_delivery(&self, delivery: Delivery) {
      println!("received message: {:?}", delivery);
      println!("data: {}", std::str::from_utf8(&delivery.data).unwrap());

      assert_eq!(delivery.data, b"Hello world!");

      self.hello_world.store(true, Ordering::SeqCst);
    }
    fn drop_prefetched_messages(&self) {}
    fn cancel(&self) {}
}

#[test]
fn connection() {
      let _ = env_logger::try_init();

      let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f?frame_max=8192".into());
      let (conn, io_loop) = addr.connect(Connection::connector(Credentials::default(), ConnectionProperties::default())).expect("io error").expect("error");

      assert_eq!(conn.status.state(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader(Credentials::default(), ConnectionProperties::default())));
      io_loop.run().expect("io loop");
      loop {
        match conn.status.state() {
          ConnectionState::Connected => break,
          state => println!("now at state {:?}, continue", state),
        }
        thread::sleep(time::Duration::from_millis(100));
      }
      println!("CONNECTED with configuration: {:?}", conn.configuration);

      //now connected

      let channel_a = conn.create_channel().unwrap();
      let channel_b = conn.create_channel().unwrap();
      //send channel
      let request_id = channel_a.channel_open().expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.status.state());
      channel_a.wait_for_reply(request_id);
      println!("[{}] state: {:?}", line!(), conn.status.state());

      //receive channel
      let request_id = channel_b.channel_open().expect("channel_open");
      println!("[{}] state: {:?}", line!(), conn.status.state());
      channel_b.wait_for_reply(request_id);
      println!("[{}] state: {:?}", line!(), conn.status.state());

      //create the hello queue
      let request_id = channel_a.queue_declare("hello-async", QueueDeclareOptions::default(), FieldTable::default()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.status.state());
      channel_a.wait_for_reply(request_id);
      println!("[{}] state: {:?}", line!(), conn.status.state());

      //purge the hello queue in case it already exists with contents in it
      let request_id = channel_a.queue_purge("hello-async", QueuePurgeOptions::default()).expect("queue_purge");
      println!("[{}] state: {:?}", line!(), conn.status.state());
      channel_a.wait_for_reply(request_id);
      println!("[{}] state: {:?}", line!(), conn.status.state());

      let request_id = channel_b.queue_declare("hello-async", QueueDeclareOptions::default(), FieldTable::default()).expect("queue_declare");
      println!("[{}] state: {:?}", line!(), conn.status.state());
      channel_b.wait_for_reply(request_id);
      println!("[{}] state: {:?}", line!(), conn.status.state());

      println!("will consume");
      let hello_world = Arc::new(AtomicBool::new(false));
      let subscriber = Subscriber { hello_world: hello_world.clone(), };
      let request_id = channel_b.basic_consume("hello-async", "my_consumer", BasicConsumeOptions::default(), FieldTable::default(), Box::new(subscriber)).expect("basic_consume");
      println!("[{}] state: {:?}", line!(), conn.status.state());
      channel_b.wait_for_reply(request_id);
      println!("[{}] state: {:?}", line!(), conn.status.state());

      println!("will publish");
      let payload = b"Hello world!";
      let request_id = channel_a.basic_publish("", "hello-async", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).expect("basic_publish");
      println!("[{}] state: {:?}", line!(), conn.status.state());
      channel_a.wait_for_reply(request_id);
      println!("[{}] state: {:?}", line!(), conn.status.state());
      thread::sleep(time::Duration::from_millis(100));
      assert!(hello_world.load(Ordering::SeqCst));
}
