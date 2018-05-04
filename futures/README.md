# lapin-futures

This library offers a futures based API over the lapin-async library.
It leverages the tokio-io and futures library, so you can use it
with tokio, futures-cpupool or any other reactor.

The library is designed so it does not own the socket, so you
can use any TCP, TLS or unix socket based stream.

Calls to the underlying stream are guarded by a mutex, so you could
use one connection from multiple threads.

There's an [example available](https://github.com/Geal/lapin/blob/master/futures/examples/client.rs)
using tokio.

## Publishing a message

```rust,no_run
#[macro_use] extern crate log;
extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio;

use futures::future::Future;
use futures::Stream;
use tokio::net::TcpStream;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicPublishOptions,BasicProperties,QueueDeclareOptions};
use lapin::types::FieldTable;

fn main() {
  let addr = "127.0.0.1:5672".parse().unwrap();

  tokio::run(
    TcpStream::connect(&addr).and_then(|stream| {

      // connect() returns a future of an AMQP Client
      // that resolves once the handshake is done
      lapin::client::Client::connect(stream, &ConnectionOptions::default())
   }).and_then(|(client, _ /* heartbeat */)| {

      // create_channel returns a future that is resolved
      // once the channel is successfully created
      client.create_channel()
    }).and_then(|channel| {
      let id = channel.id;
      info!("created channel with id: {}", id);

      // we using a "move" closure to reuse the channel
      // once the queue is declared. We could also clone
      // the channel
      channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
        info!("channel {} declared queue {}", id, "hello");

        channel.basic_publish("", "hello", b"hello from tokio", &BasicPublishOptions::default(), BasicProperties::default())
      })
    }).map(|_| ()).map_err(|_| ())
  )
}
```

## Creating a consumer

```rust,no_run
#[macro_use] extern crate log;
extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio;

use tokio::prelude::Future;
use tokio::prelude::Stream;
use tokio::net::TcpStream;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicConsumeOptions,QueueDeclareOptions};
use lapin::types::FieldTable;

fn main() {
  let addr = "127.0.0.1:5672".parse().unwrap();

  tokio::run(
    TcpStream::connect(&addr).and_then(|stream| {

      // connect() returns a future of an AMQP Client
      // that resolves once the handshake is done
      lapin::client::Client::connect(stream, &ConnectionOptions::default())
    }).and_then(|(client, heartbeat)| {
      // The heartbeat future should be run in a dedicated thread so that nothing can prevent it from
      // dispatching events on time.
      // If we ran it as part of the "main" chain of futures, we might end up not sending
      // some heartbeats if we don't poll often enough (because of some blocking task or such).
      tokio::spawn(heartbeat(&client).map_err(|_| ()));

      // create_channel returns a future that is resolved
      // once the channel is successfully created
      client.create_channel()
    }).and_then(|channel| {
      let id = channel.id;
      info!("created channel with id: {}", id);

      let ch = channel.clone();
      channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
        info!("channel {} declared queue {}", id, "hello");

        // basic_consume returns a future of a message
        // stream. Any time a message arrives for this consumer,
        // the for_each method would be called
        channel.basic_consume("hello", "my_consumer", &BasicConsumeOptions::default(), &FieldTable::new())
      }).and_then(|stream| {
        info!("got consumer stream");

        stream.for_each(move |message| {
          debug!("got message: {:?}", message);
          info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
          ch.basic_ack(message.delivery_tag);
          Ok(())
        })
      })
    }).map_err(|_| ())
  )
}
```
