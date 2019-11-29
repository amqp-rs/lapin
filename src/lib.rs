#![warn(rust_2018_idioms)]

//! This project follows the AMQP 0.9.1 specifications, targetting especially RabbitMQ.
//!
//! ## Example
//!
//! ```rust,no_run
//! use lapin::{
//!     options::*, types::FieldTable, BasicProperties, Connection,
//!     ConnectionProperties
//! };
//! use futures::{future::FutureExt, stream::StreamExt};
//! use log::info;
//! use anyhow::Result;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     env_logger::init();
//!
//!     let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
//!
//!     let conn = Connection::connect(&addr, ConnectionProperties::default())
//! 	.await
//! 	.expect("Couldn't connect to RabbitMQ");
//!
//!     info!("CONNECTED");
//!
//!     let channel_a = conn.create_channel().await?;
//!     let channel_b = conn.create_channel().await?;
//!
//!     channel_a
//!         .queue_declare(
//!             "hello",
//!             QueueDeclareOptions::default(),
//!             FieldTable::default(),
//!         )
//!         .await?;
//!
//!     let queue = channel_b
//!         .queue_declare(
//!             "hello",
//!             QueueDeclareOptions::default(),
//!             FieldTable::default(),
//!         )
//!         .await?;
//!
//!     let consumer = channel_b
//!         .clone()
//!         .basic_consume(
//!             &queue,
//!             "my_consumer",
//!             BasicConsumeOptions::default(),
//!             FieldTable::default(),
//!         )
//!         .await?;
//!
//!     tokio::spawn(async move {
//!         info!("will consume");
//!         consumer
//!             .for_each(move |delivery| {
//!                 let delivery = delivery.expect("error caught in in consumer");
//!                 channel_b
//!                     .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
//!                     .map(|_| ())
//!             })
//!             .await
//!     });
//!
//!     let payload = b"Hello world!";
//!
//!     loop {
//!         channel_a
//!             .basic_publish(
//!                 "",
//!                 "hello",
//!                 BasicPublishOptions::default(),
//!                 payload.to_vec(),
//!                 BasicProperties::default(),
//!             )
//!             .await?;
//!     }
//! }
//!
//! ```

pub use amq_protocol::{
    auth,
    protocol::{self, BasicProperties},
    tcp, types, uri,
};

pub use channel::{options, Channel};
pub use channel_status::{ChannelState, ChannelStatus};
pub use configuration::Configuration;
pub use connection::{Connect, Connection};
pub use connection_properties::ConnectionProperties;
pub use connection_status::{ConnectionState, ConnectionStatus};
pub use consumer::{Consumer, ConsumerDelegate, ConsumerIterator};
pub use error::{Error, Result};
pub use exchange::ExchangeKind;
pub use queue::Queue;

pub mod confirmation;
pub mod executor;
pub mod message;

mod acknowledgement;
mod buffer;
mod channel;
mod channel_status;
mod channels;
mod configuration;
mod connection;
mod connection_properties;
mod connection_status;
mod consumer;
mod error;
mod error_handler;
mod exchange;
mod frames;
mod id_sequence;
mod io_loop;
mod queue;
mod queues;
mod registration;
mod returned_messages;
mod wait;
