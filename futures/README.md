# lapin-futures

This library offers a futures based API over the lapin-async library.
It leverages the tokio-io and futures library, so you can use it
with tokio-core, futures-cpupool or any other reactor.

The library is designed so it does not own the socket, so you
can use any TCP, TLS or unix socket based stream.

Calls to the underlying stream are guarded by a mutex, so you could
use one connection from multiple threads.

There's an [example available](https://github.com/Geal/lapin/blob/master/futures/examples/client.rs)
using tokio-core.

## Publishing a message

```rust
#[macro_use] extern crate log;
extern crate amq_protocol;
extern crate futures;
extern crate tokio_core;
extern crate lapin_futures as lapin;

use amq_protocol::types::FieldTable;
use futures::Stream;
use futures::future::Future;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicPublishOptions,QueueDeclareOptions, BasicProperties};

fn main() {

  // create the reactor
  let mut core = Core::new().unwrap();
  let handle = core.handle();
  let addr = "127.0.0.1:5672".parse().unwrap();

  core.run(

    TcpStream::connect(&addr, &handle).and_then(|stream| {

      // connect() returns a future of an AMQP Client
      // that resolves once the handshake is done
      lapin::client::Client::connect(stream, &ConnectionOptions::default())
    }).and_then(|client| {

      // create_channel returns a future that is resolved
      // once the channel is successfully created
      client.create_channel()
    }).and_then(|channel| {
      let id = channel.id;
      info!("created channel with id: {}", id);

      // we using a "move" closure to reuse the channel
      // once the queue is declared. We could also clone
      // the channel
      channel.queue_declare("hello", &QueueDeclareOptions::default(), FieldTable::new()).and_then(move |_| {
        info!("channel {} declared queue {}", id, "hello");

          channel.basic_publish("hello", b"hello from tokio", &BasicPublishOptions::default(),
                                BasicProperties::default())
      })
    })
  ).unwrap();
}
```

## Creating a consumer

```rust
#[macro_use] extern crate log;
extern crate amq_protocol;
extern crate futures;
extern crate tokio_core;
extern crate lapin_futures as lapin;

use amq_protocol::types::FieldTable;
use futures::Stream;
use futures::future::Future;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicConsumeOptions,BasicPublishOptions,QueueDeclareOptions};
fn main() {

  // create the reactor
  let mut core = Core::new().unwrap();
  let handle = core.handle();
  let addr = "127.0.0.1:5672".parse().unwrap();

  core.run(

    TcpStream::connect(&addr, &handle).and_then(|stream| {

      // connect() returns a future of an AMQP Client
      // that resolves once the handshake is done
      lapin::client::Client::connect(stream, &ConnectionOptions::default())
    }).and_then(|client| {

      // create_channel returns a future that is resolved
      // once the channel is successfully created
      client.create_channel()
    }).and_then(|channel| {
      let id = channel.id;
      info!("created channel with id: {}", id);

      let ch = channel.clone();
      channel.queue_declare("hello", &QueueDeclareOptions::default(), FieldTable::new()).and_then(move |_| {
        info!("channel {} declared queue {}", id, "hello");

        // basic_consume returns a future of a message
        // stream. Any time a message arrives for this consumer,
        // the for_each method would be called
        channel.basic_consume("hello", "my_consumer", &BasicConsumeOptions::default())
      }).and_then(|stream| {
        info!("got consumer stream");

        stream.for_each(move|message| {
          debug!("got message: {:?}", message);
          info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
          ch.basic_ack(message.delivery_tag);
          Ok(())
        })
      })
    })
  ).unwrap();
}
```
