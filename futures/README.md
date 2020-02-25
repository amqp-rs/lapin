# lapin-futures

This library offers a futures-0.1 based API over the lapin library.
It leverages the futures-0.1 library, so you can use it
with tokio, futures-cpupool or any other executor.

## Publishing a message

```rust,no_run
use futures::future::Future;
use lapin_futures as lapin;
use crate::lapin::{BasicProperties, Client, ConnectionProperties};
use crate::lapin::options::{BasicPublishOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use log::info;

fn main() {
  env_logger::init();

  let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

  futures::executor::spawn(
   Client::connect(&addr, ConnectionProperties::default()).and_then(|client| {
      // create_channel returns a future that is resolved
      // once the channel is successfully created
      client.create_channel()
    }).and_then(|channel| {
      let id = channel.id();
      info!("created channel with id: {}", id);

      // we using a "move" closure to reuse the channel
      // once the queue is declared. We could also clone
      // the channel
      channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).and_then(move |_| {
        info!("channel {} declared queue {}", id, "hello");

        channel.basic_publish("", "hello", b"hello from tokio".to_vec(), BasicPublishOptions::default(), BasicProperties::default())
      })
    })
  ).wait_future().expect("runtime failure");
}
```

## Creating a consumer

```rust,no_run
use futures::{Future, Stream};
use lapin_futures as lapin;
use crate::lapin::{Client, ConnectionProperties};
use crate::lapin::options::{BasicConsumeOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use log::{debug, info};

fn main() {
  env_logger::init();

  let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

  futures::executor::spawn(
   Client::connect(&addr, ConnectionProperties::default()).and_then(|client| {
      // create_channel returns a future that is resolved
      // once the channel is successfully created
      client.create_channel()
    }).and_then(|channel| {
      let id = channel.id();
      info!("created channel with id: {}", id);

      let ch = channel.clone();
      channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).and_then(move |queue| {
        info!("channel {} declared queue {:?}", id, queue);

        // basic_consume returns a future of a message
        // stream. Any time a message arrives for this consumer,
        // the for_each method would be called
        channel.basic_consume("hello", "my_consumer", BasicConsumeOptions::default(), FieldTable::default())
      }).and_then(|stream| {
        info!("got consumer stream");

        stream.for_each(move |message| {
          debug!("got message: {:?}", message);
          info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
          ch.basic_ack(message.delivery_tag, false)
        })
      })
    })
  ).wait_future().expect("runtime failure");
}
```
