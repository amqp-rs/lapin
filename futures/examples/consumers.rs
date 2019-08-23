use crate::lapin::options::{BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions};
use crate::lapin::types::FieldTable;
use crate::lapin::{BasicProperties, Client, ConnectionProperties};
use futures::{Future, Stream};
use lapin_futures as lapin;
use log::info;

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

    futures::executor::spawn(
        Client::connect(&addr, ConnectionProperties::default())
            .map_err(|err| eprintln!("An error occured: {}", err))
            .map(|client| {
                for n in 0..N_CONSUMERS {
                    let _client = client.clone();
                    std::thread::spawn(move || {
                        futures::executor::spawn(create_consumer(&_client, n))
                            .wait_future()
                            .expect("consumer failure")
                    });
                }
                client
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
    .wait_future()
    .expect("runtime exited with failure");
}
