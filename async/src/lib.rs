#![warn(rust_2018_idioms)]

//! lapin-async
//!
//! this library is meant for use in an event loop. The library exposes, through the
//! [Connection struct](https://docs.rs/lapin-async/0.1.0/lapin_async/connection/struct.Connection.html),
//! a state machine you can drive through IO you manage.
//!
//! Typically, your code would own the socket and buffers, and regularly pass the
//! input and output buffers to the state machine so it receives messages and
//! serializes new ones to send. You can then query the current state and see
//! if it received new messages for the consumers.
//!
//! ## Example
//!
//! ```rust,no_run
//! use env_logger;
//! use lapin_async as lapin;
//! use log::info;
//!
//! use crate::lapin::{
//!   channel::{BasicProperties, Channel},
//!   channel::options::*,
//!   connection::Connection,
//!   connection_properties::ConnectionProperties,
//!   consumer::ConsumerSubscriber,
//!   credentials::Credentials,
//!   message::Delivery,
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
//!     self.channel.basic_ack(delivery.delivery_tag, BasicAckOptions::default()).into_result().expect("basic_ack");
//!   }
//!   fn drop_prefetched_messages(&self) {}
//!   fn cancel(&self) {}
//! }
//!
//! fn main() {
//!   env_logger::init();
//!
//!   let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
//!   let conn = Connection::connect(&addr, Credentials::default(), ConnectionProperties::default()).wait().expect("connection error");
//!
//!   info!("CONNECTED");
//!
//!   let channel_a = conn.create_channel().wait().expect("create_channel");
//!   let channel_b = conn.create_channel().wait().expect("create_channel");
//!
//!   channel_a.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).wait().expect("queue_declare");
//!   channel_b.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).wait().expect("queue_declare");
//!
//!   info!("will consume");
//!   channel_b.basic_consume("hello", "my_consumer", BasicConsumeOptions::default(), FieldTable::default(), Box::new(Subscriber { channel: channel_b.clone() })).wait().expect("basic_consume");
//!
//!   let payload = b"Hello world!";
//!
//!   loop {
//!     channel_a.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).wait().expect("basic_publish");
//!   }
//! }
//! ```

pub mod acknowledgement;
pub mod buffer;
pub mod channel;
pub mod channel_status;
pub mod channels;
pub mod confirmation;
pub mod connection;
pub mod connection_properties;
pub mod connection_status;
pub mod configuration;
pub mod consumer;
pub mod credentials;
pub mod error;
pub mod frames;
pub mod id_sequence;
pub mod io_loop;
pub mod message;
pub mod queue;
pub mod queues;
pub mod registration;
pub mod returned_messages;
pub mod tcp;
pub mod types;
pub mod uri;
pub mod wait;

pub use connection::Connect;
