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
//!
//! use std::{
//!   net::TcpStream,
//!   thread,
//!   time,
//! };
//!
//! use crate::lapin::{
//!   buffer::Buffer,
//!   channel::BasicProperties,
//!   channel_status::ChannelState,
//!   channel::options::*,
//!   connection::Connection,
//!   connection_properties::ConnectionProperties,
//!   connection_status::{ConnectionState, ConnectingState},
//!   consumer::ConsumerSubscriber,
//!   credentials::Credentials,
//!   message::Delivery,
//!   types::FieldTable,
//! };
//!
//! fn main() {
//!   env_logger::init();
//!
//!   /* Open TCP connection */
//!   let mut stream = TcpStream::connect("127.0.0.1:5672").unwrap();
//!   stream.set_nonblocking(true).unwrap();
//!
//!   /* Configure AMQP connection */
//!   let capacity = 8192;
//!   let mut send_buffer    = Buffer::with_capacity(capacity as usize);
//!   let mut receive_buffer = Buffer::with_capacity(capacity as usize);
//!   let mut conn: Connection = Connection::new();
//!   conn.configuration.set_frame_max(capacity);
//!
//!   /* Connect tp RabbitMQ server */
//!   assert_eq!(conn.connect(Credentials::default(), ConnectionProperties::default()).unwrap(), ConnectionState::Connecting(ConnectingState::SentProtocolHeader(Credentials::default(), ConnectionProperties::default())));
//!   loop {
//!     match conn.run(&mut stream, &mut send_buffer, &mut receive_buffer) {
//!       Err(e) => panic!("could not connect: {:?}", e),
//!       Ok(ConnectionState::Connected) => break,
//!       Ok(state) => println!("now at state {:?}, continue", state),
//!     }
//!     thread::sleep(time::Duration::from_millis(100));
//!   }
//!   println!("CONNECTED");
//!
//!   /* Adapt our buffer after negocation with the server */
//!   let frame_max = conn.configuration.frame_max();
//!   if frame_max > capacity {
//!     send_buffer.grow(frame_max as usize);
//!     receive_buffer.grow(frame_max as usize);
//!   }
//!
//!   /* Create and open a channel */
//!   let channel = conn.create_channel().unwrap();
//!   channel.channel_open().expect("channel_open");
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!   thread::sleep(time::Duration::from_millis(100));
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!   assert!(channel.status.state() == ChannelState::Connected);
//!
//!   /* Declaire the "hellp" queue */
//!   let request_id = channel.queue_declare("hello", QueueDeclareOptions::default(), FieldTable::default()).unwrap();
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!   thread::sleep(time::Duration::from_millis(100));
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!   assert!(channel.requests.was_successful(request_id.unwrap()).unwrap_or(false));
//!
//!   /* Publish "Hellow world!" to the "hello" queue */
//!   let payload = b"Hello world!";
//!   channel.basic_publish("", "hello", BasicPublishOptions::default(), payload.to_vec(), BasicProperties::default()).expect("basic_publish");
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!   thread::sleep(time::Duration::from_millis(100));
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!
//!   /* Consumer the messages from the "hello" queue using an instance of Subscriber */
//!   let request_id = channel.basic_consume("hello", "my_consumer", BasicConsumeOptions::default(), FieldTable::default(), Box::new(Subscriber)).expect("basic_consume");
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!   thread::sleep(time::Duration::from_millis(100));
//!   conn.run(&mut stream, &mut send_buffer, &mut receive_buffer).unwrap();
//!   assert!(channel.requests.was_successful(request_id.unwrap()).unwrap_or(false));
//! }
//!
//! #[derive(Debug)]
//! struct Subscriber;
//!
//! impl ConsumerSubscriber for Subscriber {
//!   fn new_delivery(&self, _delivery: Delivery) {
//!     // handle message
//!   }
//!   fn drop_prefetched_messages(&self) {}
//!   fn cancel(&self) {}
//! }
//! ```

pub mod acknowledgement;
pub mod buffer;
pub mod channel;
pub mod channel_status;
pub mod channels;
pub mod connection;
pub mod connection_properties;
pub mod connection_status;
pub mod configuration;
pub mod consumer;
pub mod credentials;
pub mod error;
pub mod generated_names;
pub mod id_sequence;
pub mod io;
pub mod message;
pub mod priority_frames;
pub mod queue;
pub mod queues;
pub mod replies;
pub mod requests;
pub mod types;
pub mod uri;
