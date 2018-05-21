#[macro_use] extern crate log;
extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio;
extern crate env_logger;

use futures::Stream;
use futures::future::Future;
use tokio::net::TcpStream;

use lapin::types::FieldTable;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicConsumeOptions,BasicPublishOptions,BasicQosOptions,BasicProperties,QueueDeclareOptions,QueueDeleteOptions,QueuePurgeOptions};

#[test]
fn connection() {
  env_logger::init();

  let addr = "127.0.0.1:5672".parse().unwrap();

  tokio::run(
    TcpStream::connect(&addr).and_then(|stream| {
      lapin::client::Client::connect(stream, &ConnectionOptions::default())
    }).and_then(|(client, _)| {

      client.create_channel().and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);

        channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
          info!("channel {} declared queue {}", id, "hello");

          channel.queue_purge("hello", &QueuePurgeOptions::default()).and_then(move |_| {
            channel.basic_publish("", "hello", b"hello from tokio", &BasicPublishOptions::default(), BasicProperties::default())
          })
        })
      }).and_then(move |_| {
        client.create_channel()
      }).and_then(|channel| {
        let id = channel.id;
        info!("created channel with id: {}", id);

        let ch1 = channel.clone();
        let ch2 = channel.clone();
        channel.basic_qos(&BasicQosOptions { prefetch_count: 16, ..Default::default() }).and_then(move |_| {
          info!("channel QoS specified");
          channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).map(move |_| channel)
        }).and_then(move |channel| {
          info!("channel {} declared queue {}", id, "hello");

          channel.basic_consume("hello", "my_consumer", &BasicConsumeOptions::default(), &FieldTable::new())
        }).and_then(move |stream| {
          info!("got consumer stream");

          stream.into_future().map_err(|(err, _)| err).and_then(move |(message, _)| {
            let msg = message.unwrap();
            info!("got message: {:?}", msg);
            assert_eq!(msg.data, b"hello from tokio");
            ch1.basic_ack(msg.delivery_tag)
          }).and_then(move |_| {
            ch2.queue_delete("hello", &QueueDeleteOptions::default())
          })
        })
      })
    }).map_err(|_| ())
  )
}
