//! lapin-futures
//!
//! This library offers a futures based API over the lapin-async library.
//! It leverages the tokio-io and futures library, so you can use it
//! with tokio-core, futures-cpupool or any other reactor.
//!
//! The library is designed so it does not own the socket, so you
//! can use any TCP, TLS or unix socket based stream.
//!
//! Calls to the underlying stream are guarded by a mutex, so you could
//! use one connection from multiple threads.
//!
//! There's an [example available](https://github.com/Geal/lapin/blob/master/futures/examples/client.rs)
//! using tokio-core.
//!
//! ## Publishing a message
//!
//! ```rust,no_run
//! #[macro_use] extern crate log;
//! extern crate lapin_futures as lapin;
//! extern crate futures;
//! extern crate tokio_core;
//!
//! use futures::future::Future;
//! use futures::Stream;
//! use tokio_core::reactor::Core;
//! use tokio_core::net::TcpStream;
//! use lapin::client::ConnectionOptions;
//! use lapin::channel::{BasicPublishOptions,BasicProperties,QueueDeclareOptions};
//! use lapin::types::FieldTable;
//!
//! fn main() {
//!
//!   // create the reactor
//!   let mut core = Core::new().unwrap();
//!   let handle = core.handle();
//!   let addr = "127.0.0.1:5672".parse().unwrap();
//!
//!   core.run(
//!
//!     TcpStream::connect(&addr, &handle).and_then(|stream| {
//!
//!       // connect() returns a future of an AMQP Client
//!       // that resolves once the handshake is done
//!       lapin::client::Client::connect(stream, &ConnectionOptions::default())
//!    }).and_then(|(client, _ /* heartbeat_future_fn */)| {
//!
//!       // create_channel returns a future that is resolved
//!       // once the channel is successfully created
//!       client.create_channel()
//!     }).and_then(|channel| {
//!       let id = channel.id;
//!       info!("created channel with id: {}", id);
//!
//!       // we using a "move" closure to reuse the channel
//!       // once the queue is declared. We could also clone
//!       // the channel
//!       channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
//!         info!("channel {} declared queue {}", id, "hello");
//!
//!         channel.basic_publish("", "hello", b"hello from tokio", &BasicPublishOptions::default(), BasicProperties::default())
//!       })
//!     })
//!   ).unwrap();
//! }
//! ```
//!
//! ## Creating a consumer
//!
//! ```rust,no_run
//! #[macro_use] extern crate log;
//! extern crate lapin_futures as lapin;
//! extern crate futures;
//! extern crate tokio_core;
//!
//! use futures::future::Future;
//! use futures::Stream;
//! use tokio_core::reactor::Core;
//! use tokio_core::net::TcpStream;
//! use lapin::client::ConnectionOptions;
//! use lapin::channel::{BasicConsumeOptions,BasicPublishOptions,QueueDeclareOptions};
//! use lapin::types::FieldTable;
//! use std::thread;
//!
//! fn main() {
//!
//!   // create the reactor
//!   let mut core = Core::new().unwrap();
//!   let handle = core.handle();
//!   let addr = "127.0.0.1:5672".parse().unwrap();
//!
//!   core.run(
//!
//!     TcpStream::connect(&addr, &handle).and_then(|stream| {
//!
//!       // connect() returns a future of an AMQP Client
//!       // that resolves once the handshake is done
//!       lapin::client::Client::connect(stream, &ConnectionOptions::default())
//!    }).and_then(|(client, heartbeat_future_fn)| {
//!      // The heartbeat future should be run in a dedicated thread so that nothing can prevent it from
//!      // dispatching events on time.
//!      // If we ran it as part of the "main" chain of futures, we might end up not sending
//!      // some heartbeats if we don't poll often enough (because of some blocking task or such).
//!      let heartbeat_client = client.clone();
//!      thread::Builder::new().name("heartbeat thread".to_string()).spawn(move || {
//!        Core::new().unwrap().run(heartbeat_future_fn(&heartbeat_client)).unwrap();
//!      }).unwrap();
//!
//!       // create_channel returns a future that is resolved
//!       // once the channel is successfully created
//!       client.create_channel()
//!     }).and_then(|channel| {
//!       let id = channel.id;
//!       info!("created channel with id: {}", id);
//!
//!       let ch = channel.clone();
//!       channel.queue_declare("hello", &QueueDeclareOptions::default(), &FieldTable::new()).and_then(move |_| {
//!         info!("channel {} declared queue {}", id, "hello");
//!
//!         // basic_consume returns a future of a message
//!         // stream. Any time a message arrives for this consumer,
//!         // the for_each method would be called
//!         channel.basic_consume("hello", "my_consumer", &BasicConsumeOptions::default(), &FieldTable::new())
//!       }).and_then(|stream| {
//!         info!("got consumer stream");
//!
//!         stream.for_each(move |message| {
//!           debug!("got message: {:?}", message);
//!           info!("decoded message: {:?}", std::str::from_utf8(&message.data).unwrap());
//!           ch.basic_ack(message.delivery_tag);
//!           Ok(())
//!         })
//!       })
//!     })
//!   ).unwrap();
//! }
//! ```
//!

extern crate amq_protocol;
extern crate cookie_factory;
extern crate bytes;
#[macro_use] extern crate futures;
extern crate lapin_async;
#[macro_use] extern crate log;
extern crate nom;
extern crate tokio_io;
extern crate tokio_timer;

pub mod client;
pub mod transport;
pub mod channel;
pub mod consumer;
pub mod types;
