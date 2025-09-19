#![warn(rust_2018_idioms)]

//! lapin
//!
//! This project follows the AMQP 0.9.1 specifications, targeting especially RabbitMQ.
//!
//! The main access point is the [`Channel`], which contains the individual
//! AMQP methods. As to the AMQP specification, one TCP [`Connection`] can contain
//! multiple channels.
//!
//! ## Feature switches
//!
//! * `codegen`: generate code instead of using pregenerated one
//! * `native-tls`: enable amqps support through native-tls (preferred over rustls when set)
//! * `openssl`: enable amqps support through openssl (preferred over rustls when set)
//! * `rustls` (*default*): enable amqps support through rustls (uses rustls-native-certs by default)
//! * `rustls-native-certs`: same as rustls, be ensure we'll still use rustls-native-certs even if the default for rustls changes
//! * `rustls-webpki-roots-certs`: same as rustls but using webkit-roots instead of rustls-native-certs
//!
//! ## Example
//!
//! ```rust,no_run
//! use async_rs::traits::*;
//! use futures_lite::stream::StreamExt;
//! use lapin::{
//!     options::*, Confirmation, types::FieldTable, BasicProperties, Connection,
//!     ConnectionProperties, Result,
//! };
//! use tracing::info;
//!
//! fn main() -> Result<()> {
//!     if std::env::var("RUST_LOG").is_err() {
//!         unsafe { std::env::set_var("RUST_LOG", "info") };
//!     }
//!
//!     tracing_subscriber::fmt::init();
//!
//!     let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
//!     let runtime = lapin::runtime::default_runtime()?;
//!
//!     runtime.clone().block_on(async move {
//!         let conn = Connection::connect_with_runtime(
//!             &addr,
//!             ConnectionProperties::default(),
//!             runtime.clone(),
//!         )
//!         .await?;
//!
//!         info!("CONNECTED");
//!
//!         let channel_a = conn.create_channel().await?;
//!         let channel_b = conn.create_channel().await?;
//!
//!         let queue = channel_a
//!             .queue_declare(
//!                 "hello".into(),
//!                 QueueDeclareOptions::default(),
//!                 FieldTable::default(),
//!             )
//!             .await?;
//!
//!         info!(?queue, "Declared queue");
//!
//!         let mut consumer = channel_b
//!             .basic_consume(
//!                 "hello".into(),
//!                 "my_consumer".into(),
//!                 BasicConsumeOptions::default(),
//!                 FieldTable::default(),
//!             )
//!             .await?;
//!         runtime.spawn(async move {
//!             info!("will consume");
//!             while let Some(delivery) = consumer.next().await {
//!                 let delivery = delivery.expect("error in consumer");
//!                 delivery
//!                     .ack(BasicAckOptions::default())
//!                     .await
//!                     .expect("ack");
//!             }
//!         });
//!
//!         let payload = b"Hello world!";
//!
//!         loop {
//!             let confirm = channel_a
//!                 .basic_publish(
//!                     "".into(),
//!                     "hello".into(),
//!                     BasicPublishOptions::default(),
//!                     payload,
//!                     BasicProperties::default(),
//!                 )
//!                 .await?
//!                 .await?;
//!             assert_eq!(confirm, Confirmation::NotRequested);
//!         }
//!     })
//! }
//! ```
//! [`Channel`]: ./struct.Channel.html
//! [`Connection`]: ./struct.Connection.html

pub use amq_protocol::{
    protocol::{self, BasicProperties},
    tcp::{self, AsyncTcpStream},
    types, uri,
};

pub use acker::Acker;
pub use channel::{Channel, options};
pub use channel_status::{ChannelState, ChannelStatus};
pub use configuration::Configuration;
pub use connection::{Connect, Connection};
pub use connection_properties::ConnectionProperties;
pub use connection_status::{ConnectionState, ConnectionStatus};
pub use consumer::{Consumer, ConsumerDelegate};
pub use error::{Error, ErrorKind, Result};
pub use events::Event;
pub use exchange::ExchangeKind;
pub use publisher_confirm::Confirmation;
pub use queue::Queue;
pub use recovery_config::RecoveryConfig;

pub mod auth;
pub mod message;
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
mod connection_closer;
mod connection_properties;
mod connection_status;
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
mod notifier;
mod parsing;
mod promise;
mod publisher_confirm;
mod queue;
mod recovery_config;
mod registry;
mod returned_messages;
mod secret_update;
mod socket_state;
mod thread;
mod topology;
mod wakers;
