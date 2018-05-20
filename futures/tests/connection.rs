#[macro_use] extern crate log;
extern crate lapin_futures as lapin;
extern crate futures;
extern crate tokio;
extern crate env_logger;

use std::net::SocketAddr;

use futures::Stream;
use futures::future::Future;
use tokio::net::TcpStream;

use lapin::types::FieldTable;
use lapin::client::ConnectionOptions;
use lapin::channel::{BasicConsumeOptions,BasicPublishOptions,BasicQosOptions,BasicProperties,QueueDeclareOptions,QueueDeleteOptions,QueuePurgeOptions};

fn amqp_addr() -> SocketAddr {
  std::env::var("AMQP_ADDR")
    .unwrap_or("127.0.0.1:5672".to_string())
    .parse()
    .unwrap()
}

#[test]
fn connection() {
  // Ignore error initializing logger; other tests might have done it already.
  let _ = env_logger::try_init();

  tokio::run(
    TcpStream::connect(&amqp_addr()).and_then(|stream| {
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
          channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).map(move |()| channel)
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

#[test]
fn consume_before_publish() {
  use std::time::{Duration, Instant};
  use tokio::timer::Delay;
  use tokio::util::FutureExt;

  // Ignore error initializing logger; other tests might have done it already.
  let _ = env_logger::try_init();

  let connect_future = TcpStream::connect(&amqp_addr()).and_then(|stream| {
    lapin::client::Client::connect(stream, &ConnectionOptions::default())
  });

  tokio::run(
    connect_future.and_then(|(client, _heartbeat_future_fn)| {
      // Create a channel and use it to declare and purge a queue for this test.
      let create_and_purge_queue_future = client.create_channel().and_then(|create_purge_channel| {
        let id = create_purge_channel.id;
        info!("created channel with id: {}", id);

        create_purge_channel
          .queue_declare(
            "hello_again",
            &QueueDeclareOptions::default(),
            &FieldTable::new(),
          ).and_then(move |_| {
            // Purge the queue so we don't end up with messages from previous test runs!
            create_purge_channel.queue_purge(
              "hello_again",
              &QueuePurgeOptions::default(),
            ).map(move |_| {
              info!("channel {} declared queue {}", id, "hello_again");
            })
          })
      });

      // Create a channel and use it to consume messages.
      // The result of the future will be a single Delivery.
      let consume_one_message_future = client.create_channel().and_then(|consume_channel| {
        let id = consume_channel.id;
        info!("created channel with id: {}", id);
        let ack_channel = consume_channel.clone();

        let deadline_when = Instant::now() + Duration::from_millis(1000);
        consume_channel.basic_consume(
          "hello_again",
          "my_consumer",
          &BasicConsumeOptions::default(),
          &FieldTable::new(),
        ).and_then(|stream| {
          info!("got consumer stream");

          // We've purged the queue, and we're only sending one message below.
          stream.into_future()
            .map(|(maybe_message, _stream)| maybe_message.expect("Should have been at least one message..."))
            .map_err(|(err, _stream)| err)
        })
        .and_then(move |message| {
          info!("got message: {:?}", message);
          info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
          ack_channel.basic_ack(message.delivery_tag).map(move |_| {
            message
          })
        })
        .deadline(deadline_when)
        .map_err(|err| {
          panic!("Didn't receive a message in time: {:?}", err);
        })
      });

      // Create channel and use it to publish messages.
      let publish_future = client.create_channel().and_then(move |publish_channel| {
        let id = publish_channel.id;
        info!("created channel with id: {}", id);
        publish_channel
          .basic_publish(
            "",
            "hello_again",
            b"please receive me",
            &BasicPublishOptions::default(),
            BasicProperties::default()
              .with_user_id("guest".to_string())
              .with_reply_to("foobar".to_string()),
          )
          .map(|confirmation| {
            info!(
              "publish got confirmation: {:?}",
              confirmation
            )
          })
      });

      // (JP:) Both "Example 1" and "Example 2" below should work,
      // but don't.
      //
      // Prior to some refactoring that I don't believe should
      // have made any difference, "Example 2" worked. I guess this
      // is the same issue as the original problem: tasks not
      // always getting notified when necessary.
      //
      // Note that neither of these causes the test to fail;
      // they just panic in a worker thread. If a solution is found
      // to the underlying problem, I'll happily write a thorough
      // test suite demonstrating a bunch of different orders of
      // operations that should work, structured to be a bit more
      // easily readable than this. :)
      //
      // Maybe if we get lucky (?) then https://github.com/rust-lang/rust/pull/50850
      // will land first, and all of this will become beautiful!

      // Example 1: Consume before publish.
      create_and_purge_queue_future.and_then({|_|
        consume_one_message_future.and_then(|_| {
          // Wait a little bit to make sure that we really
          // do start consuming first.
          let when = Instant::now() + Duration::from_millis(100);
          Delay::new(when)
            .map_err(|e| panic!("timer failed; err={:?}", e))
            .and_then(move |_| {
              publish_future
            })
        })
      })

      // Example 2: Publish before consume.
      // create_and_purge_queue_future.and_then(|_| {
      //   publish_future.and_then(|_| {
      //     let when = Instant::now() + Duration::from_millis(100);
      //     Delay::new(when)
      //       .map_err(|e| panic!("timer failed; err={:?}", e))
      //       .and_then(move |_| {
      //         consume_one_message_future.map(|_| ())
      //       })
      //   })
      // })
    }).map_err(|err| {
      panic!("uh oh: {:?}", err);
    })
  );
}
