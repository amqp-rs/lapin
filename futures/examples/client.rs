// Long and nested future chains can quickly result in large generic types.
#![type_length_limit="16777216"]

use env_logger;
use failure::Error;
use futures::{future::Future, Stream};
use lapin_futures as lapin;
use crate::lapin::channel::{BasicConsumeOptions, BasicGetOptions, BasicPublishOptions, BasicProperties, ConfirmSelectOptions, ExchangeBindOptions, ExchangeUnbindOptions, ExchangeDeclareOptions, ExchangeDeleteOptions, QueueBindOptions, QueueDeclareOptions};
use crate::lapin::client::ConnectionOptions;
use crate::lapin::types::FieldTable;
use log::{debug, info};
use tokio;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;

fn main() {
  env_logger::init();

  let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "127.0.0.1:5672".to_string()).parse().unwrap();

  Runtime::new().unwrap().block_on_all(
    TcpStream::connect(&addr).map_err(Error::from).and_then(|stream| {
      lapin::client::Client::connect(stream, ConnectionOptions {
        frame_max: 65535,
        ..Default::default()
      }).map_err(Error::from)
    }).and_then(|(client, heartbeat)| {
      tokio::spawn(heartbeat.map_err(|e| eprintln!("heartbeat error: {}", e)));

      client.create_confirm_channel(ConfirmSelectOptions::default()).and_then(|mut channel| {
        let id = channel.id();
        info!("created channel with id: {}", id);

        channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::new()).and_then(move |_| {
          info!("channel {} declared queue {}", id, "hello");

          channel.exchange_declare("hello_exchange", "direct", ExchangeDeclareOptions::default(), FieldTable::new()).and_then(move |_| {
            channel.queue_bind("hello", "hello_exchange", "hello_2", QueueBindOptions::default(), FieldTable::new()).and_then(move |_| {
              channel.basic_publish(
                "hello_exchange",
                "hello_2",
                b"hello from tokio".to_vec(),
                BasicPublishOptions::default(),
                BasicProperties::default().with_user_id("guest".to_string()).with_reply_to("foobar".to_string())
              ).map(|confirmation| {
                info!("publish got confirmation: {:?}", confirmation)
              }).and_then(move |_| {
                channel.exchange_bind("hello_exchange", "amq.direct", "test_bind", ExchangeBindOptions::default(), FieldTable::new()).and_then(move |_| {
                    channel.exchange_unbind("hello_exchange", "amq.direct", "test_bind", ExchangeUnbindOptions::default(), FieldTable::new()).and_then(move |_| {
                        channel.exchange_delete("hello_exchange", ExchangeDeleteOptions::default()).and_then(move |_| {
                            channel.close(200, "Bye")
                        })
                    })
                })
              })
            })
          })
        })
      }).and_then(move |_| {
        client.create_channel()
      }).and_then(|mut channel| {
        let id = channel.id();
        info!("created channel with id: {}", id);

        let mut c = channel.clone();
        channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::new()).and_then(move |queue| {
          info!("channel {} declared queue {:?}", id, queue);

          let mut ch = channel.clone();
          channel.basic_get("hello", BasicGetOptions::default()).and_then(move |message| {
            info!("got message: {:?}", message);
            info!("decoded message: {:?}", std::str::from_utf8(&message.delivery.data).unwrap());
            channel.basic_ack(message.delivery.delivery_tag, false)
          }).and_then(move |_| {
            ch.basic_consume(&queue, "my_consumer", BasicConsumeOptions::default(), FieldTable::new())
          })
        }).and_then(|stream| {
          info!("got consumer stream");

          stream.for_each(move |message| {
            debug!("got message: {:?}", message);
            info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
            c.basic_ack(message.delivery_tag, false)
          })
        })
      }).map_err(Error::from)
    }).map_err(|err| eprintln!("An error occured: {}", err))
  ).expect("runtime exited with failure")
}
