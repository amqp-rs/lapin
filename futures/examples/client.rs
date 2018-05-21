#[macro_use] extern crate log;
extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio;
extern crate env_logger;

use futures::future::Future;
use futures::Stream;
use tokio::net::TcpStream;
use lapin::types::FieldTable;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicConsumeOptions,BasicGetOptions,BasicPublishOptions,BasicProperties,ConfirmSelectOptions,ExchangeBindOptions,ExchangeUnbindOptions,ExchangeDeclareOptions,ExchangeDeleteOptions,QueueBindOptions,QueueDeclareOptions};

fn main() {
  env_logger::init();

  let addr = "127.0.0.1:5672".parse().unwrap();

  tokio::run(
    TcpStream::connect(&addr).and_then(|stream| {
      lapin::client::Client::connect(stream, &ConnectionOptions {
        frame_max: 65535,
        ..Default::default()
      })
    }).and_then(|(client, heartbeat)| {
      tokio::spawn(heartbeat.map_err(|e| eprintln!("{:?}", e)));

      client.create_confirm_channel(ConfirmSelectOptions::default()).and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);

        channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
          info!("channel {} declared queue {}", id, "hello");

          channel.exchange_declare("hello_exchange", "direct", &ExchangeDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
            channel.queue_bind("hello", "hello_exchange", "hello_2", &QueueBindOptions::default(), &FieldTable::new()).and_then(move |_| {
              channel.basic_publish(
                "hello_exchange",
                "hello_2",
                b"hello from tokio",
                &BasicPublishOptions::default(),
                BasicProperties::default().with_user_id("guest".to_string()).with_reply_to("foobar".to_string())
              ).map(|confirmation| {
                info!("publish got confirmation: {:?}", confirmation)
              }).and_then(move |_| {
                channel.exchange_bind("hello_exchange", "amq.direct", "test_bind", &ExchangeBindOptions::default(), &FieldTable::new()).and_then(move |_| {
                    channel.exchange_unbind("hello_exchange", "amq.direct", "test_bind", &ExchangeUnbindOptions::default(), &FieldTable::new()).and_then(move |_| {
                        channel.exchange_delete("hello_exchange", &ExchangeDeleteOptions::default()).and_then(move |_| {
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
      }).and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);

        let c = channel.clone();
        channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
          info!("channel {} declared queue {}", id, "hello");

          let ch = channel.clone();
          channel.basic_get("hello", &BasicGetOptions::default()).and_then(move |message| {
            info!("got message: {:?}", message);
            info!("decoded message: {:?}", std::str::from_utf8(&message.delivery.data).unwrap());
            channel.basic_ack(message.delivery.delivery_tag)
          }).and_then(move |_| {
            ch.basic_consume("hello", "my_consumer", &BasicConsumeOptions::default(), &FieldTable::new())
          })
        }).and_then(|stream| {
          info!("got consumer stream");

          stream.for_each(move |message| {
            debug!("got message: {:?}", message);
            info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
            c.basic_ack(message.delivery_tag)
          })
        })
      })
    }).map(|_| ()).map_err(|err| eprintln!("error: {:?}", err))
  )
}
