#![warn(rust_2018_idioms)]

//! lapin
//!
//! This project follows the AMQP 0.9.1 specifications, targetting especially RabbitMQ.
//!
//! ## Example
//!
//! ```rust,no_run
//! use env_logger;
//! use log::info;
//!
//! use crate::lapin::{
//!   BasicProperties, Channel, Connection, ConnectionProperties, ConsumerSubscriber,
//!   message::Delivery,
//!   options::*,
//!   types::FieldTable,
//! };
//!
//! #[derive(Clone,Debug)]
//! struct Subscriber {
//!   channel: Channel,
//! }
//!
//! impl ConsumerSubscriber for Subscriber {
//!   fn new_delivery(&self, delivery: Delivery) {
//!     self.channel.basic_ack(delivery.delivery_tag, BasicAckOptions::default()).as_error().expect("basic_ack");
//!   }
//!   fn drop_prefetched_messages(&self) {}
//!   fn cancel(&self) {}
//! }
//!
//! fn main() {
//!   env_logger::init();
//!
//!   let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
//!   let conn = Connection::connect(&addr, ConnectionProperties::default()).wait().expect("connection error");
//!
//!   info!("CONNECTED");
//!
//!   let channel_a = conn.create_channel().wait().expect("create_channel");
//!   let channel_b = conn.create_channel().wait().expect("create_channel");
//!
//!   channel_a.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).wait().expect("queue_declare");
//!   let queue = channel_b.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).wait().expect("queue_declare");
//!
//!   info!("will consume");
//!   channel_b.basic_consume(&queue, "my_consumer", BasicConsumeOptions::default(), FieldTable::default(), Box::new(Subscriber { channel: channel_b.clone() })).wait().expect("basic_consume");
//!
//!   let payload = b"Hello world!";
//!
//!   loop {
//!     channel_a.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).wait().expect("basic_publish");
//!   }
//! }
//! ```

pub use amq_protocol::{
  protocol::{self, BasicProperties},
  auth, tcp, types, uri,
};

pub use channel::{Channel, options};
pub use channel_status::{ChannelState, ChannelStatus};
pub use configuration::Configuration;
pub use connection::{Connect, Connection};
pub use connection_properties::ConnectionProperties;
pub use connection_status::{ConnectionState, ConnectionStatus};
pub use consumer::ConsumerSubscriber;
pub use error::{Error, ErrorKind};
pub use queue::Queue;

pub mod confirmation;
pub mod message;
pub mod streaming_consumer;

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
mod frames;
mod id_sequence;
mod io_loop;
mod queue;
mod queues;
mod registration;
mod returned_messages;
mod wait;
