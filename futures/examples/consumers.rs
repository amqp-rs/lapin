extern crate env_logger;
extern crate lapin_futures as lapin;
#[macro_use]
extern crate log;
extern crate failure;
extern crate futures;
extern crate tokio;

use failure::{err_msg, Error};
use futures::future::Future;
use futures::{IntoFuture, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use lapin::types::FieldTable;
use lapin::client::{Client, ConnectionOptions};
use lapin::channel::{BasicConsumeOptions, BasicProperties, BasicPublishOptions, ConfirmSelectOptions, QueueDeclareOptions};

const N_CONSUMERS : u8 = 8;
const N_MESSAGES  : u8 = 5;

fn create_consumer<T: AsyncRead + AsyncWrite + Sync + Send + 'static>(client: &Client<T>, n: u8) -> impl Future<Item = (), Error = ()> + Send + 'static {
    info!("will create consumer {}", n);

    let queue = format!("test-queue-{}", n);

    client.create_confirm_channel(ConfirmSelectOptions::default()).and_then(move |channel| {
        info!("creating queue {}", queue);
        channel.queue_declare(&queue, QueueDeclareOptions::default(), FieldTable::new()).map(move |queue| (channel, queue))
    }).and_then(move |(channel, queue)| {
        info!("creating consumer {}", n);
        channel.basic_consume(&queue, "", BasicConsumeOptions::default(), FieldTable::new()).map(move |stream| (channel, stream))
    }).and_then(move |(channel, stream)| {
        info!("got stream for consumer {}", n);
        stream.for_each(move |message| {
            println!("consumer '{}' got '{}'", n, std::str::from_utf8(&message.data).unwrap());
            channel.basic_ack(message.delivery_tag, false)
        })
    }).map(|_| ()).map_err(move |err| eprintln!("got error in consumer '{}': {}", n, err))
}

fn main() {
    env_logger::init();

    let addr    = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "127.0.0.1:5672".to_string()).parse().unwrap();
    let runtime = Runtime::new().unwrap();
    // let mut runtime = tokio::runtime::current_thread::Runtime::new().unwrap();

    runtime.block_on_all(
        TcpStream::connect(&addr).map_err(Error::from).and_then(|stream| {
            Client::connect(stream, ConnectionOptions {
                frame_max: 65535,
                heartbeat: 20,
                ..Default::default()
            }).map_err(Error::from)
        }).and_then(|(client, heartbeat)| {
            tokio::spawn(heartbeat.map_err(|e| eprintln!("heartbeat error: {:?}", e)))
                .into_future().map(|_| client).map_err(|_| err_msg("spawn error"))
        }).and_then(|client| {
            let _client = client.clone();
            futures::stream::iter_ok(0..N_CONSUMERS).for_each(move |n| tokio::spawn(create_consumer(&_client, n)))
                .into_future().map(move |_| client).map_err(|_| err_msg("spawn error"))
        }).and_then(|client| {
            client.create_confirm_channel(ConfirmSelectOptions::default()).and_then(move |channel| {
                futures::stream::iter_ok((0..N_CONSUMERS).flat_map(|c| {
                    (0..N_MESSAGES).map(move |m| (c, m))
                })).for_each(move |(c, m)| {
                    let queue   = format!("test-queue-{}", c);
                    let message = format!("message {} for consumer {}", m, c);
                    let channel = channel.clone();

                    info!("will publish {}", message);

                    channel.queue_declare(&queue, QueueDeclareOptions::default(), FieldTable::new()).and_then(move |_| {
                        channel.basic_publish("", &queue, message.into_bytes(), BasicPublishOptions::default(), BasicProperties::default()).map(move |confirmation| {
                            println!("got confirmation (consumer {}, message {}): {:?}", c, m, confirmation);
                        })
                    })
                })
            }).map_err(Error::from)
        }).map_err(|err| eprintln!("error: {:?}", err))
    ).expect("runtime exited with failure");
}
