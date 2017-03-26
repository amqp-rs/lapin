#[macro_use] extern crate log;
extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio_core;
extern crate env_logger;

use futures::future::Future;
use futures::Stream;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicConsumeOptions,BasicGetOptions,BasicPublishOptions,BasicProperties,QueueDeclareOptions};

fn main() {
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

          channel.basic_publish("hello", b"hello from tokio", &BasicPublishOptions::default(), BasicProperties::default())
        })
      }).and_then(move |_| {
        client.create_channel()
      }).and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);

        let c = channel.clone();
        channel.queue_declare("hello", &QueueDeclareOptions::default()).and_then(move |_| {
          info!("channel {} declared queue {}", id, "hello");

          channel.basic_consume("hello", "my_consumer", &BasicConsumeOptions::default())
        }).and_then(|stream| {
          info!("got consumer stream");

          stream.for_each(move |message| {
            debug!("got message: {:?}", message);
            info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
            c.basic_ack(message.delivery_tag);
            Ok(())
          })
        })
      })
    })
  ).unwrap();
}
