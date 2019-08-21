use crate::lapin::options::{BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use crate::lapin::{BasicProperties, Client, ConnectionProperties};
use env_logger;
use futures;
use futures::{Future, IntoFuture, Stream};
use lapin_futures as lapin;
use log::info;
use tokio;
use tokio::runtime::Runtime;

const N_CONSUMERS: u8 = 8;
const N_MESSAGES: u8 = 5;

fn create_consumer(client: &Client, n: u8) -> impl Future<Item = (), Error = ()> + Send + 'static {
    info!("will create consumer {}", n);

    let queue = format!("test-queue-{}", n);

    client
        .create_channel()
        .and_then(move |channel| {
            info!("creating queue {}", queue);
            channel
                .queue_declare(
                    &queue,
                    QueueDeclareOptions::default(),
                    FieldTable::default(),
                )
                .map(move |queue| (channel, queue))
        })
        .and_then(move |(channel, queue)| {
            info!("creating consumer {}", n);
            channel
                .basic_consume(
                    &queue,
                    "",
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .map(move |stream| (channel, stream))
        })
        .and_then(move |(channel, stream)| {
            info!("got stream for consumer {}", n);
            stream.for_each(move |message| {
                println!(
                    "consumer '{}' got '{}'",
                    n,
                    std::str::from_utf8(&message.data).unwrap()
                );
                channel.basic_ack(message.delivery_tag, false)
            })
        })
        .map(|_| ())
        .map_err(move |err| eprintln!("got error in consumer '{}': {}", n, err))
}

fn main() {
    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let runtime = Runtime::new().unwrap();
    // let runtime = tokio::runtime::current_thread::Runtime::new().unwrap();

    runtime
        .block_on_all(
            Client::connect(&addr, ConnectionProperties::default())
                .map_err(|err| eprintln!("An error occured: {}", err))
                .and_then(|client| {
                    let _client = client.clone();
                    futures::stream::iter_ok(0..N_CONSUMERS)
                        .for_each(move |n| tokio::spawn(create_consumer(&_client, n)))
                        .into_future()
                        .map(move |_| client)
                })
                .and_then(|client| {
                    client
                        .create_channel()
                        .map_err(|err| eprintln!("An error occured: {}", err))
                        .and_then(move |channel| {
                            futures::stream::iter_ok(
                                (0..N_CONSUMERS).flat_map(|c| (0..N_MESSAGES).map(move |m| (c, m))),
                            )
                            .for_each(move |(c, m)| {
                                let queue = format!("test-queue-{}", c);
                                let message = format!("message {} for consumer {}", m, c);
                                let channel = channel.clone();

                                info!("will publish {}", message);

                                channel
                                    .queue_declare(
                                        &queue,
                                        QueueDeclareOptions::default(),
                                        FieldTable::default(),
                                    )
                                    .and_then(move |_| {
                                        channel
                                            .basic_publish(
                                                "",
                                                &queue,
                                                message.into_bytes(),
                                                BasicPublishOptions::default(),
                                                BasicProperties::default(),
                                            )
                                            .map(|_| ())
                                    })
                            })
                            .map_err(|err| eprintln!("An error occured: {}", err))
                        })
                }),
        )
        .expect("runtime exited with failure");
}
