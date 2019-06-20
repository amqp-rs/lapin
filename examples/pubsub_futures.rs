#![feature(async_await)]

use env_logger;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use lapin::{
  BasicProperties, Connection, ConnectionProperties, Error,
  options::*,
  types::FieldTable,
};
use log::info;

#[runtime::main]
async fn main() -> Result<(), Error> {
  env_logger::init();

  let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
  let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;

  info!("CONNECTED");

  let channel_a = conn.create_channel().await?;
  let channel_b = conn.create_channel().await?;

  channel_a.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).await?;
  let queue = channel_b.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).await?;

  let consumer  = channel_b.clone().basic_consume_streaming(&queue, "my_consumer", BasicConsumeOptions::default(), FieldTable::default()).await?;
  let _consumer = runtime::spawn(async move {
    info!("will consume");
    consumer.for_each(move |delivery| {
      channel_b.basic_ack(delivery.delivery_tag, BasicAckOptions::default()).map(|_| ())
    }).await
  });

  let payload   = b"Hello world!";

  loop {
    channel_a.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).await?;
  }
}
