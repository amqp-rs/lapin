// Long and nested future chains can quickly result in large generic types.
#![type_length_limit="16777216"]

use env_logger;
use failure::Error;
use futures::{Future, Stream};
use lapin_futures as lapin;
use crate::lapin::{BasicProperties, ConnectionProperties, Client};
use crate::lapin::options::{BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use log::{debug, info};
use tokio;
use tokio::runtime::Runtime;

fn main() {
  env_logger::init();

  let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

  Runtime::new().unwrap().block_on_all(
    Client::connect(&addr, ConnectionProperties::default()).map_err(Error::from).and_then(|client| {
      let publisher = client.create_channel().and_then(|pub_channel| {
        let id = pub_channel.id();
        info!("created publisher channel with id: {}", id);

        pub_channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).and_then(move |_| {
          info!("publisher channel {} declared queue {}", id, "hello");
          futures::stream::repeat(b"hello".to_vec()).for_each(move |msg| {
            pub_channel.basic_publish(
              "",
              "hello",
              msg,
              BasicPublishOptions::default(),
              BasicProperties::default().with_user_id("guest".into()).with_reply_to("foobar".into())
            ).map(|_| ())
          })
        })
      });

      tokio::spawn(publisher.map_err(|_| ()));

      client.create_channel().and_then(|sub_channel| {
        let id = sub_channel.id();
        info!("created subscriber channel with id: {}", id);

        let ch = sub_channel.clone();

        sub_channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).and_then(move |queue| {
          info!("subscriber channel {} declared queue {}", id, "hello");
          sub_channel.basic_consume(&queue, "my_consumer", BasicConsumeOptions::default(), FieldTable::default())
        }).and_then(|stream| {
          info!("got consumer stream");

          stream.for_each(move |message| {
            debug!("got message: {:?}", message);
            info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
            ch.basic_ack(message.delivery_tag, false)
          })
        })
      }).map_err(Error::from)
    }).map_err(|err| eprintln!("An error occured: {}", err))
  ).expect("runtime exited with failure")
}
