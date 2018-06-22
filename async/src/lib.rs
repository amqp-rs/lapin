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
//! ## Creating a new connection
//!
//! Set up an AMQP 0.9.1 compliant server. For the purpose of this documentation,
//! we'll assume it is listening on `127.0.0.1:5672`.
//!
//! Create the client socket and some buffers to move data:
//!
//! ```rust,ignore
//! extern crate lapin_async as lapin;
//!
//! use std::net::TcpStream;
//! use lapin::connection::*;
//! use lapin::buffer::Buffer;
//!
//! fn main() {
//!   let mut stream = TcpStream::connect("127.0.0.1:5672").unwrap();
//!   stream.set_nonblocking(true);
//!
//!   let capacity = 8192;
//!   let mut send_buffer    = Buffer::with_capacity(capacity as usize);
//!   let mut receive_buffer = Buffer::with_capacity(capacity as usize);
//!
//! }
//! ```
//!
//! Now, we can create the `Connection` object:
//!
//! ```rust,ignore
//!   let mut conn: Connection = Connection::new();
//!   conn.set_frame_max(capacity);
//!   loop {
//!     match conn.run(&mut stream, &mut send_buffer, &mut receive_buffer) {
//!       Err(e) => panic!("could not connect: {:?}", e),
//!         Ok(ConnectionState::Connected) => break,
//!         Ok(state) => println!("now at state {:?}, continue", state),
//!       }
//!     thread::sleep(time::Duration::from_millis(100));
//!   }
//!   println!("CONNECTED");
//!
//!   let frame_max = conn.configuration.frame_max;
//!   if frame_max > capacity {
//!     send_buffer.grow(frame_max as usize);
//!     receive_buffer.grow(frame_max as usize);
//!   }
//!
//! ```
//!
//! The [`run` method](https://docs.rs/lapin-async/0.1.0/lapin_async/connection/struct.Connection.html#method.run)
//! will repeatedly call 3 other methods:
//!
//! - [`read_from_stream`](https://docs.rs/lapin-async/0.1.0/lapin_async/connection/struct.Connection.html#method.read_from_stream) with the input buffer. This method is only there as a helper to get data from the network into a `Buffer` struct, you might want to handle reading yourself
//! - [`parse`](https://docs.rs/lapin-async/0.1.0/lapin_async/connection/struct.Connection.html#method.parse) will parse the data you just gathered, update the state, and return the current state and how much bytes were consumed
//! - then you must call [`write_to_stream`](https://docs.rs/lapin-async/0.1.0/lapin_async/connection/struct.Connection.html#method.write_to_stream) in case the state machine must send messages to the server. It returns how much data was consumed and the current state
//!
//! Calling `parse` and `write_to_stream` is how you handle the plumbing for the state machine.
//! Your code should then react to the state changes returned by those functions, or to the
//! specific state of channels and consumers.
//!
//! In the current case, we wait until the state machine gets to the `ConnectionState::Connected` state.
//!
//! ## Creating a channel
//!
//! ```rust,ignore
//! let channel_id: u16 = conn.create_channel().unwrap();
//! conn.channel_open(channel_a, "".to_string()).expect("channel_open");
//!
//! // update state here until:
//! assert!(conn.check_state(channel_id, ChannelState::Connected).unwrap_or(false));
//! ```
//!
//! ## Creating a queue
//!
//! ```rust,ignore
//! //create the "hello" queue
//! let request_id: u16 = conn.queue_declare(channel_id, 0, "hello".to_string(), false, false, false, false, false, HashMap::new()).unwrap();
//!
//! // update state here until:
//! assert!(conn.is_finished(request_id).unwrap_or(false));
//! ```
//!
//! ## Publishing a message
//!
//! ```rust,ignore
//! conn.basic_publish(channel_id, 0, "".to_string(), "hello".to_string(), false, false).expect("basic_publish");
//! let payload = b"Hello world!";
//! conn.send_content_frames(channel_a, 60, payload, basic::Properties::default()));
//!
//! // update state
//! ```
//!
//! ## Creating a Consumer
//!
//! ```rust,ignore
//! //create the "hello" queue
//! let request_id: u16 = conn.basic_consume(channel_id, 0, "hello".to_string(), "my_consumer".to_string(), false, true, false, false, HashMap::new()).expect("basic_consume");
//!
//! // update state here until:
//! assert!(conn.is_finished(request_id).unwrap_or(false));
//!
//! // get the next message
//! if let Ok(message) = conn.next_message(channel_id, "hello", "my_consumer") {
//!  // handle message
//! }
//! ```

extern crate amq_protocol;
#[macro_use]
extern crate log;
extern crate nom;
extern crate cookie_factory;
extern crate sasl;

pub mod buffer;
pub mod io;
pub mod connection;
pub mod consumer;
pub mod channel;
pub mod queue;
pub mod message;
pub mod api;
pub mod error;
pub mod types;
