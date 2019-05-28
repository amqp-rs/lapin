use env_logger;
use lapin_async as lapin;
use log::info;

use std::{
  thread,
  time,
};

use crate::lapin::{
  channel::{BasicProperties, Channel},
  channel::options::*,
  connection::Connection,
  connection_properties::ConnectionProperties,
  connection_status::ConnectionState,
  consumer::ConsumerSubscriber,
  credentials::Credentials,
  message::Delivery,
  tcp::AMQPUriTcpExt,
  types::FieldTable,
};

#[derive(Clone,Debug)]
struct Subscriber {
  channel: Channel,
}

impl ConsumerSubscriber for Subscriber {
    fn new_delivery(&self, delivery: Delivery) {
      self.channel.basic_ack(delivery.delivery_tag, BasicAckOptions::default()).expect("basic_ack");
    }
    fn drop_prefetched_messages(&self) {}
    fn cancel(&self) {}
}

fn main() {
      env_logger::init();

      let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f?frame_max=8192".into());
      let (conn, io_loop) = addr.connect(Connection::connector(Credentials::default(), ConnectionProperties::default())).expect("io error").expect("error");

      io_loop.run().expect("io loop");

      loop {
        match conn.status.state() {
          ConnectionState::Connected => break,
          state => info!("now at state {:?}, continue", state),
        }
        thread::sleep(time::Duration::from_millis(100));
      }
      info!("CONNECTED");

      let channel_b = conn.create_channel().unwrap();
      thread::Builder::new().name("consumer".to_string()).spawn(move || {
        let request_id = channel_b.channel_open().expect("channel_open");
        channel_b.wait_for_reply(request_id);
        let request_id = channel_b.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).expect("queue_declare");
        channel_b.wait_for_reply(request_id);

        info!("will consume");
        let request_id = channel_b.basic_consume("hello", "my_consumer", BasicConsumeOptions::default(), FieldTable::default(), Box::new(Subscriber { channel: channel_b.clone() })).expect("basic_consume");
        channel_b.wait_for_reply(request_id);
      }).expect("consumer thread");

      let channel_a = conn.create_channel().unwrap();

      let request_id = channel_a.channel_open().expect("channel_open");
      channel_a.wait_for_reply(request_id);
      let request_id = channel_a.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).expect("queue_declare");
      channel_a.wait_for_reply(request_id);

      let payload = b"Hello world!";

      loop {
        let request_id = channel_a.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).expect("basic_publish");
        channel_a.wait_for_reply(request_id);
      }
}

