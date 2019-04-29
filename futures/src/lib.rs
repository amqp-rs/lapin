#![warn(rust_2018_idioms)]

//! lapin-futures
//!
//! This library offers a futures based API over the lapin-async library.
//! It leverages the tokio-io and futures library, so you can use it
//! with tokio, futures-cpupool or any other reactor.
//!
//! The library is designed so it does not own the socket, so you
//! can use any TCP, TLS or unix socket based stream.
//!
//! Calls to the underlying stream are guarded by a mutex, so you could
//! use one connection from multiple threads.
//!
//! There's an [example available](https://github.com/sozu-proxy/lapin/blob/master/futures/examples/client.rs)
//! using tokio.
//!
//! ## Publishing a message
//!
//! ```rust,no_run
//! use env_logger;
//! use failure::Error;
//! use futures::future::Future;
//! use lapin_futures as lapin;
//! use crate::lapin::channel::{BasicPublishOptions, BasicProperties, QueueDeclareOptions};
//! use crate::lapin::client::ConnectionOptions;
//! use crate::lapin::types::FieldTable;
//! use log::info;
//! use tokio;
//! use tokio::net::TcpStream;
//! use tokio::runtime::Runtime;
//!
//! fn main() {
//!   env_logger::init();
//!
//!   let addr = "127.0.0.1:5672".parse().unwrap();
//!
//!   Runtime::new().unwrap().block_on_all(
//!     TcpStream::connect(&addr).map_err(Error::from).and_then(|stream| {
//!
//!       // connect() returns a future of an AMQP Client
//!       // that resolves once the handshake is done
//!       lapin::client::Client::connect(stream, ConnectionOptions::default()).map_err(Error::from)
//!    }).and_then(|(client, _ /* heartbeat */)| {
//!
//!       // create_channel returns a future that is resolved
//!       // once the channel is successfully created
//!       client.create_channel().map_err(Error::from)
//!     }).and_then(|mut channel| {
//!       let id = channel.id();
//!       info!("created channel with id: {}", id);
//!
//!       // we using a "move" closure to reuse the channel
//!       // once the queue is declared. We could also clone
//!       // the channel
//!       channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::new()).and_then(move |_| {
//!         info!("channel {} declared queue {}", id, "hello");
//!
//!         channel.basic_publish("", "hello", b"hello from tokio".to_vec(), BasicPublishOptions::default(), BasicProperties::default())
//!       }).map_err(Error::from)
//!     })
//!   ).expect("runtime failure");
//! }
//! ```
//!
//! ## Creating a consumer
//!
//! ```rust,no_run
//! use env_logger;
//! use failure::Error;
//! use futures::{future::Future, Stream};
//! use lapin_futures as lapin;
//! use crate::lapin::client::ConnectionOptions;
//! use crate::lapin::channel::{BasicConsumeOptions, QueueDeclareOptions};
//! use crate::lapin::types::FieldTable;
//! use log::{debug, info};
//! use tokio;
//! use tokio::net::TcpStream;
//! use tokio::runtime::Runtime;
//!
//! fn main() {
//!   env_logger::init();
//!
//!   let addr = "127.0.0.1:5672".parse().unwrap();
//!
//!   Runtime::new().unwrap().block_on_all(
//!     TcpStream::connect(&addr).map_err(Error::from).and_then(|stream| {
//!
//!       // connect() returns a future of an AMQP Client
//!       // that resolves once the handshake is done
//!       lapin::client::Client::connect(stream, ConnectionOptions::default()).map_err(Error::from)
//!    }).and_then(|(client, heartbeat)| {
//!      // The heartbeat future should be run in a dedicated thread so that nothing can prevent it from
//!      // dispatching events on time.
//!      // If we ran it as part of the "main" chain of futures, we might end up not sending
//!      // some heartbeats if we don't poll often enough (because of some blocking task or such).
//!      tokio::spawn(heartbeat.map_err(|_| ()));
//!
//!       // create_channel returns a future that is resolved
//!       // once the channel is successfully created
//!       client.create_channel().map_err(Error::from)
//!     }).and_then(|mut channel| {
//!       let id = channel.id();
//!       info!("created channel with id: {}", id);
//!
//!       let mut ch = channel.clone();
//!       channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::new()).and_then(move |queue| {
//!         info!("channel {} declared queue {}", id, "hello");
//!
//!         // basic_consume returns a future of a message
//!         // stream. Any time a message arrives for this consumer,
//!         // the for_each method would be called
//!         channel.basic_consume(&queue, "my_consumer", BasicConsumeOptions::default(), FieldTable::new())
//!       }).and_then(|stream| {
//!         info!("got consumer stream");
//!
//!         stream.for_each(move |message| {
//!           debug!("got message: {:?}", message);
//!           info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
//!           ch.basic_ack(message.delivery_tag, false)
//!         })
//!       }).map_err(Error::from)
//!     })
//!   ).expect("runtime failure");
//! }
//! ```

pub mod channel;
pub mod client;
pub mod consumer;
pub mod error;
pub mod message;
pub mod queue;
pub mod transport;
pub mod types;
pub mod uri;
