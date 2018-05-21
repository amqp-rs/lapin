extern crate env_logger;
extern crate lapin_futures as lapin;
#[macro_use]
extern crate log;
extern crate futures;
extern crate tokio;

use futures::future::Future;
use futures::Stream;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use lapin::types::FieldTable;
use lapin::client::{Client, ConnectionOptions};
use lapin::channel::{BasicConsumeOptions, BasicProperties, BasicPublishOptions, ConfirmSelectOptions, QueueDeclareOptions};

fn create_consumer<T: AsyncRead + AsyncWrite + Sync + Send + 'static>(client: &Client<T>, n: u8) -> Box<Future<Item = (), Error = ()> + Send + 'static> {
    info!("will create consumer {}", n);

    let queue = format!("test-queue-{}", n);

    Box::new(
        client.create_confirm_channel(ConfirmSelectOptions::default()).and_then(move |channel| {
            channel.queue_declare(&queue, &QueueDeclareOptions::default(), &FieldTable::new()).map(move |_| (channel, queue.clone()))
        }).and_then(move |(channel, queue)| {
            info!("creating consumer {}", n);
            channel.basic_consume(&queue, "", &BasicConsumeOptions::default(), &FieldTable::new()).map(move |stream| (channel, stream))
        }).and_then(move |(channel, stream)| {
            info!("got stream for consumer {}", n);
            stream.for_each(move |message| {
                println!("consumer '{}' got '{}'", n, std::str::from_utf8(&message.data).unwrap());
                channel.basic_ack(message.delivery_tag)
            })
        }).map(|_| ()).map_err(move |err| eprintln!("got error in consumer '{}': {:?}", n, err))
    )
}

fn main() {
    env_logger::init();

    let addr = "127.0.0.1:5672".parse().unwrap();

    tokio::run(
        TcpStream::connect(&addr).and_then(|stream| {
            Client::connect(stream, &ConnectionOptions {
                frame_max: 65535,
                ..Default::default()
            })
        }).and_then(|(client, heartbeat)| {
            tokio::spawn(heartbeat.map_err(|e| eprintln!("heartbeat error: {:?}", e)));

            for n in 1..9 {
                tokio::spawn(create_consumer(&client, n));
            }

            client.create_confirm_channel(ConfirmSelectOptions::default()).and_then(move |channel| {
                futures::stream::iter_ok((1..9).flat_map(|c| {
                    (1..6).map(move |m| (c, m))
                })).for_each(move |(c, m)| {
                    let queue   = format!("test-queue-{}", c);
                    let message = format!("message {} for consumer {}", m, c);
                    let channel = channel.clone();

                    info!("will publish {}", message);

                    channel.queue_declare(&queue, &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
                        channel.basic_publish("", &queue, message.as_str().as_bytes(), &BasicPublishOptions::default(), BasicProperties::default()).map(move |confirmation| {
                            println!("got confirmation (consumer {}, message {}): {:?}", c, m, confirmation);
                        })
                    })
                })
            })
        }).map(|_| ()).map_err(|err| eprintln!("error: {:?}", err))
    );
}
