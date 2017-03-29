#[macro_use] extern crate log;
extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio_core;
extern crate env_logger;

use futures::Stream;
use futures::future::Future;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;

use lapin::client::ConnectionOptions;
use lapin::channel::{BasicConsumeOptions,BasicPublishOptions,BasicProperties,QueueDeclareOptions};

#[test]
fn connection() {
  env_logger::init().unwrap();
  let mut core = Core::new().unwrap();

  let handle = core.handle();
  let addr = "127.0.0.1:5672".parse().unwrap();

  core.run(
    TcpStream::connect(&addr, &handle).and_then(|stream| {
      lapin::client::Client::connect(stream, &ConnectionOptions::default())
    }).and_then(|client| {

      client.create_channel().and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);

        channel.queue_declare("hello", &QueueDeclareOptions::default()).and_then(move |_| {
          info!("channel {} declared queue {}", id, "hello");

          channel.queue_purge("hello").and_then(move |_| {
            channel.basic_publish("hello", b"hello from tokio", &BasicPublishOptions::default(), BasicProperties::default())
          })
        })
      }).and_then(move |_| {
        client.create_channel()
      }).and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);

        let ch = channel.clone();
        channel.queue_declare("hello", &QueueDeclareOptions::default()).and_then(move |_| {
          info!("channel {} declared queue {}", id, "hello");

          channel.basic_consume("hello", "my_consumer", &BasicConsumeOptions::default())
        }).and_then(move |stream| {
          info!("got consumer stream");

          stream.into_future().and_then(move |(message, _)| {
            let msg = message.unwrap();
            info!("got message: {:?}", msg);
            assert_eq!(msg.data, b"hello from tokio");
            ch.basic_ack(msg.delivery_tag);
            Ok(())
          }).map_err(|(err, _)| err)
        })
      })
    })
  ).unwrap();
}
