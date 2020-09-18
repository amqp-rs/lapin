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
//! * `native-tls` (*default*): enable amqps support through native-tls
//! * `openssl`: enable amqps support through openssl (preferred over native-tls when set)
//! * `rustls`: enable amqps support through rustls (preferred over openssl when set, uses rustls-native-certs by default)
//! * `rustls-native-certs`: same as rustls, be ensure we'll still use rustls-native-certs even if the default for rustls changes
//! * `rustls-webpki-roots-certs`: same as rustls but using webkit-roots instead of rustls-native-certs
//!
//! ## Example
//!
//! ```rust,no_run
//! use futures_lite::stream::StreamExt;
//! use lapin::{
//!     options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
//!     ConnectionProperties, Result,
//! };
//! use tracing::info;
//!
//! fn main() -> Result<()> {
//!     if std::env::var("RUST_LOG").is_err() {
//!         std::env::set_var("RUST_LOG", "info");
//!     }
//!
//!     tracing_subscriber::fmt::init();
//!
//!     let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
//!
//!     async_global_executor::block_on(async {
//!         let conn = Connection::connect(
//!             &addr,
//!             ConnectionProperties::default(),
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
//!                 "hello",
//!                 QueueDeclareOptions::default(),
//!                 FieldTable::default(),
//!             )
//!             .await?;
//!
//!         info!("Declared queue {:?}", queue);
//!
//!         let mut consumer = channel_b
//!             .basic_consume(
//!                 "hello",
//!                 "my_consumer",
//!                 BasicConsumeOptions::default(),
//!                 FieldTable::default(),
//!             )
//!             .await?;
//!         async_global_executor::spawn(async move {
//!             info!("will consume");
//!             while let Some(delivery) = consumer.next().await {
//!                 let (channel, delivery) = delivery.expect("error in consumer");
//!                 channel
//!                     .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
//!                     .await
//!                     .expect("ack");
//!             }
//!         }).detach();
//!
//!         let payload = b"Hello world!";
//!
//!         loop {
//!             let confirm = channel_a
//!                 .basic_publish(
//!                     "",
//!                     "hello",
//!                     BasicPublishOptions::default(),
//!                     payload.to_vec(),
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
pub use stream::TcpStream;

pub mod executor;
pub mod heartbeat;
pub mod message;
pub mod publisher_confirm;
pub mod reactor;
pub mod socket_state;

type Promise<T> = pinky_swear::PinkySwear<Result<T>>;
type PromiseResolver<T> = pinky_swear::Pinky<Result<T>>;

mod acknowledgement;
mod buffer;
mod channel;
mod channel_closer;
mod channel_receiver_state;
mod channel_status;
mod channels;
mod configuration;
mod connection;
mod connection_closer;
mod connection_properties;
mod connection_status;
mod consumer;
mod error;
mod error_handler;
mod exchange;
mod frames;
mod id_sequence;
mod internal_rpc;
mod io_loop;
mod parsing;
mod queue;
mod queues;
mod returned_messages;
mod stream;
mod thread;
