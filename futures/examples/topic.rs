extern crate env_logger;
extern crate failure;
extern crate lapin_futures as lapin;
extern crate log;
extern crate futures;
extern crate tokio;

use failure::{err_msg, Error};
use futures::future::Future;
use futures::IntoFuture;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use lapin::types::FieldTable;
use lapin::client::{Client, ConnectionOptions};
use lapin::channel::{BasicProperties, BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions};

fn main() {
    env_logger::init();

    let addr    = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "127.0.0.1:5672".to_string()).parse().unwrap();
    let runtime = Runtime::new().unwrap();

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
            client.create_confirm_channel(ConfirmSelectOptions::default()).map_err(Error::from)
        }).and_then(|channel| {
            channel.clone().exchange_declare("hello_topic", "topic", ExchangeDeclareOptions::default(), FieldTable::new()).map(move |_| channel).map_err(Error::from)
        }).and_then(|channel| {
            channel.clone().queue_declare("topic_queue", QueueDeclareOptions::default(), FieldTable::new()).map(move |_| channel).map_err(Error::from)
        }).and_then(|channel| {
            channel.clone().queue_bind("topic_queue", "hello_topic", "*.foo.*", QueueBindOptions::default(), FieldTable::new()).map(move |_| channel).map_err(Error::from)
        }).and_then(|channel| {
            channel.basic_publish("hello_topic", "hello.fooo.bar", b"hello".to_vec(), BasicPublishOptions::default(), BasicProperties::default()).map(|confirmation| {
                println!("got confirmation of publication: {:?}", confirmation);
            }).map_err(Error::from)
        }).map_err(|err| eprintln!("error: {:?}", err))
    ).expect("runtime exited with failure");
}
