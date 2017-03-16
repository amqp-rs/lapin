extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio_core;
extern crate env_logger;

use futures::future::Future;
use futures::Stream;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;

fn main() {
  env_logger::init().unwrap();
  let mut core = Core::new().unwrap();

  let handle = core.handle();
  let addr = "127.0.0.1:5672".parse().unwrap();

  core.run(
    TcpStream::connect(&addr, &handle).and_then(|stream| {
      lapin::client::Client::connect(stream)
    }).and_then(|client| {

      client.create_channel().and_then(|channel| {
        let id = channel.id;
        println!("created channel with id: {}", id);

        channel.queue_declare("hello").and_then(move |_| {
          println!("channel {} declared queue {}", id, "hello");

          channel.basic_publish("hello", b"hello from tokio")
        })
      }).and_then(move |_| {
        client.create_channel()
      }).and_then(|channel| {
        let id = channel.id;
        println!("created channel with id: {}", id);

        channel.queue_declare("hello").and_then(move |_| {
          println!("channel {} declared queue {}", id, "hello");

          channel.basic_consume("hello", "my_consumer")
        }).and_then(|stream| {
          println!("got consumer stream");

          stream.for_each(|message| {
            println!("got message: {:?}", message);
            println!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
            Ok(())
          })
        })
      })
    })
  ).unwrap();
}
