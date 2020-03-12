#![warn(rust_2018_idioms)]

//! lapin
//!
//! This project follows the AMQP 0.9.1 specifications, targetting especially RabbitMQ.
//!
//! ## Feature switches
//!
//! * `futures`: enable std::future::Future and async/await compatibility
//! * `native-tls` (*default*): enable amqps support through native-tls
//! * `openssl`: enable amqps support through openssl (preferred over native-tls when set)
//! * `rustls`: enable amqps support through rustls (preferred over openssl when set, uses rustls-native-certs by default)
//! * `rustls-native-certs`: same as rustls, be ensure we'll still use rustls-native-certs even if the default for rustls changes
//! * `rustls-webpki-roots-certs`: same as rustls but using webkit-roots instead of rustls-native-certs
//!
//! ## Example
//!
//! ```rust,no_run
//! use crate::lapin::{
//!   BasicProperties, Channel, Connection, ConnectionProperties, ConsumerDelegate,
//!   message::DeliveryResult,
//!   options::*,
//!   types::FieldTable,
//! };
//! use log::info;
//!
//! #[derive(Clone,Debug)]
//! struct Subscriber {
//!   channel: Channel,
//! }
//!
//! impl ConsumerDelegate for Subscriber {
//!   fn on_new_delivery(&self, delivery: DeliveryResult) {
//!     if let Some(delivery) = delivery.unwrap() {
//!       self.channel.basic_ack(delivery.delivery_tag, BasicAckOptions::default()).wait().expect("basic_ack");
//!     }
//!   }
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
//!   let queue = channel_a.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).wait().expect("queue_declare");
//!   info!("Declared queue {:?}", queue);
//!
//!   info!("will consume");
//!   channel_b.clone().basic_consume("hello", "my_consumer", BasicConsumeOptions::default(), FieldTable::default()).wait().expect("basic_consume").set_delegate(Box::new(Subscriber { channel: channel_b }));
//!
//!   let payload = b"Hello world!";
//!
//!   loop {
//!     channel_a.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).wait().expect("basic_publish");
//!   }
//! }
//! ```

pub use amq_protocol::{
    auth,
    protocol::{self, BasicProperties},
    tcp, types, uri,
};
pub use pinky_swear;

pub use channel::{options, Channel};
pub use channel_status::{ChannelState, ChannelStatus};
pub use configuration::Configuration;
pub use connection::{Connect, Connection, ConnectionPromise};
pub use connection_properties::ConnectionProperties;
pub use connection_status::{ConnectionState, ConnectionStatus};
pub use consumer::{Consumer, ConsumerDelegate, ConsumerIterator};
pub use error::{Error, Result};
pub use exchange::ExchangeKind;
pub use queue::Queue;

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
mod returned_messages;
mod waker;
