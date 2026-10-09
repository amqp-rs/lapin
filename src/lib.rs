#![deny(missing_docs, missing_debug_implementations, unsafe_code)]
#![warn(unreachable_pub, unused_qualifications, unused_lifetimes)]
#![warn(
    clippy::must_use_candidate,
    clippy::unwrap_in_result,
    clippy::panic_in_result_fn
)]

//! An async AMQP 0-9-1 client library targeting RabbitMQ.
//!
//! # Core concepts
//!
//! **[`Connection`]** — a single TCP socket to the broker. One process
//! typically creates one connection and reuses it throughout its lifetime.
//!
//! **[`Channel`]** — a lightweight virtual connection multiplexed over a
//! [`Connection`]. All AMQP operations (declaring queues, publishing,
//! consuming, …) are performed through channels. Open as many as you need;
//! they are cheap.
//!
//! **[`Consumer`]** — an async `Stream` of [`message::Delivery`] values
//! obtained by calling [`Channel::basic_consume`]. Each delivery must be
//! explicitly acknowledged once processed.
//!
//! **[`PublisherConfirm`]** — a future returned by [`Channel::basic_publish`]
//! that resolves to a [`Confirmation`] once the broker has acknowledged the
//! message (requires [`Channel::confirm_select`]).
//!
//! # Quick start
//!
//! ```rust,no_run
//! use async_rs::traits::Executor;
//! use futures_lite::stream::StreamExt;
//! use lapin::{
//!     options::*, types::FieldTable, BasicProperties, Connection,
//!     ConnectionProperties, Result,
//! };
//!
//! fn main() -> Result<()> {
//!     let addr = std::env::var("AMQP_ADDR")
//!         .unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
//!     let runtime = lapin::runtime::default_runtime()?;
//!
//!     runtime.clone().block_on(async move {
//!         let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
//!
//!         let channel = conn.create_channel().await?;
//!
//!         channel
//!             .queue_declare("hello".into(), QueueDeclareOptions::durable(), FieldTable::default())
//!             .await?;
//!
//!         channel
//!             .basic_publish(
//!                 "".into(),
//!                 "hello".into(),
//!                 BasicPublishOptions::default(),
//!                 b"Hello, world!",
//!                 BasicProperties::default(),
//!             )
//!             .await?
//!             .await?;
//!
//!         let mut consumer = channel
//!             .basic_consume(
//!                 "hello".into(),
//!                 "my_consumer".into(),
//!                 BasicConsumeOptions::default(),
//!                 FieldTable::default(),
//!             )
//!             .await?;
//!
//!         while let Some(delivery) = consumer.next().await {
//!             let delivery = delivery?;
//!             delivery.ack(BasicAckOptions::default()).await?;
//!         }
//!         Ok(())
//!     })
//! }
//! ```
//!
//! # Automatic connection recovery
//!
//! Enable recovery in [`ConnectionProperties`] to automatically reconnect and
//! replay topology (exchanges, queues, bindings, QoS settings, consumers) after
//! a network failure:
//!
//! ```rust,no_run
//! use lapin::ConnectionProperties;
//!
//! let props = ConnectionProperties::default().enable_auto_recover();
//! // then pass `props` to Connection::connect(…)
//! ```
//!
//! After catching an error from a channel operation, call
//! [`Channel::wait_for_recovery`] to block until the connection has been
//! re-established:
//!
//! ```rust,no_run
//! # use lapin::{Channel, Error, Result};
//! # async fn example(channel: Channel, error: Error) -> Result<()> {
//! channel.wait_for_recovery(error).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Feature flags
//!
//! ## Async runtime (pick exactly one)
//!
//! | Flag | Notes |
//! |------|-------|
//! | `tokio` *(default)* | Requires a Tokio runtime |
//! | `smol` | Uses the smol executor |
//! | `async-global-executor` | Uses async-global-executor |
//!
//! ## TLS backend (pick at most one; `rustls` is the default)
//!
//! | Flag | Notes |
//! |------|-------|
//! | `rustls` *(default)* | TLS via rustls |
//! | `native-tls` | TLS via the platform's native library |
//! | `openssl` | TLS via OpenSSL |
//!
//! ## Rustls certificate store (only when `rustls` is active)
//!
//! | Flag | Notes |
//! |------|-------|
//! | `rustls-platform-verifier` *(default)* | Uses the platform trust store |
//! | `rustls-native-certs` | Loads native root certificates |
//! | `rustls-webpki-roots-certs` | Uses the webpki bundled root set |
//!
//! ## Rustls crypto provider (at least one must be enabled)
//!
//! | Flag | Notes |
//! |------|-------|
//! | `rustls--aws_lc_rs` *(default)* | Uses aws-lc-rs |
//! | `rustls--ring` | Uses ring (more portable) |
//!
//! ## Miscellaneous
//!
//! | Flag | Notes |
//! |------|-------|
//! | `hickory-dns` | Use hickory-dns for name resolution |
//! | `codegen` | Force code regeneration at build time |
//! | `verbose-errors` | More detailed AMQP parser error messages |

pub use amq_protocol::{
    protocol::{self, BasicProperties},
    tcp::{self, AsyncTcpStream},
    types, uri,
};

pub use acker::Acker;
pub use channel::{Channel, options};
pub use channel_status::{ChannelQos, ChannelState, ChannelStatus};
pub use configuration::Configuration;
pub use connection::{Connect, Connection};
pub use connection_builder::{ConnectionBuilder, DefaultConnectionBuilder};
pub use connection_properties::ConnectionProperties;
pub use connection_status::{ConnectionState, ConnectionStatus};
pub use consumer::{Consumer, ConsumerDelegate};
pub use error::{Error, ErrorKind, Result};
pub use events::Event;
pub use exchange::ExchangeKind;
pub use publisher_confirm::{Confirmation, PublisherConfirm};
pub use queue::Queue;

/// Authentication providers and helpers for connecting to RabbitMQ.
pub mod auth;
/// AMQP message types delivered to consumers.
pub mod message;
/// Runtime selection and helpers.
pub mod runtime;

use promise::{Promise, PromiseResolver};

mod acker;
mod acknowledgement;
mod basic_get_delivery;
mod buffer;
mod channel;
mod channel_closer;
mod channel_receiver_state;
mod channel_recovery_context;
mod channel_status;
mod channels;
mod configuration;
mod connection;
mod connection_builder;
mod connection_closer;
mod connection_properties;
mod connection_status;
mod connection_step;
mod consumer;
mod consumer_canceler;
mod consumer_status;
mod consumers;
mod error;
mod error_holder;
mod events;
mod exchange;
mod frames;
mod future;
mod heartbeat;
mod id_sequence;
mod internal_rpc;
mod io_loop;
mod killswitch;
mod listener;
mod notifier;
mod parsing;
mod promise;
mod publisher_confirm;
mod queue;
mod registry;
mod returned_messages;
mod secret_update;
mod shared;
mod socket_state;
mod thread;
mod topology;
