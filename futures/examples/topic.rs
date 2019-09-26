use crate::lapin::options::{
    BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use crate::lapin::types::FieldTable;
use crate::lapin::{BasicProperties, Client, ConnectionProperties, ExchangeKind};
use futures::Future;
use lapin_futures as lapin;

fn main() {
    env_logger::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    futures::executor::spawn(
        Client::connect(&addr, ConnectionProperties::default())
            .and_then(|client| {
                client
                    .create_channel()
                    .and_then(|channel| {
                        channel
                            .clone()
                            .exchange_declare(
                                "hello_topic",
                                ExchangeKind::Topic,
                                ExchangeDeclareOptions::default(),
                                FieldTable::default(),
                            )
                            .map(move |_| channel)
                    })
                    .and_then(|channel| {
                        channel
                            .clone()
                            .queue_declare(
                                "topic_queue",
                                QueueDeclareOptions::default(),
                                FieldTable::default(),
                            )
                            .map(move |_| channel)
                    })
                    .and_then(|channel| {
                        channel
                            .clone()
                            .queue_bind(
                                "topic_queue",
                                "hello_topic",
                                "*.foo.*",
                                QueueBindOptions::default(),
                                FieldTable::default(),
                            )
                            .map(move |_| channel)
                    })
                    .and_then(|channel| {
                        channel.basic_publish(
                            "hello_topic",
                            "hello.fooo.bar",
                            b"hello".to_vec(),
                            BasicPublishOptions::default(),
                            BasicProperties::default(),
                        )
                    })
            })
            .map_err(|err| eprintln!("An error occured: {}", err)),
    )
    .wait_future()
    .expect("runtime exited with failure");
}
