#![warn(rust_2018_idioms)]

//! lapin
//!
//! This project follows the AMQP 0.9.1 specifications, targeting especially RabbitMQ.
//!
//! ## Feature switches
//!
//! * `native-tls` (*default*): enable amqps support through native-tls
//! * `openssl`: enable amqps support through openssl (preferred over native-tls when set)
//! * `rustls`: enable amqps support through rustls (preferred over openssl when set, uses rustls-native-certs by default)
//! * `rustls-native-certs`: same as rustls, be ensure we'll still use rustls-native-certs even if the default for rustls changes
//! * `rustls-webpki-roots-certs`: same as rustls but using webkit-roots instead of rustls-native-certs
//!
//! ## Example
//!
//! ```rust,no_run
//! use futures_executor::LocalPool;
//! use futures_util::{future::FutureExt, stream::StreamExt, task::LocalSpawnExt};
//! use lapin::{
//!     options::*, publisher_confirm::Confirmation, types::FieldTable, BasicProperties, Connection,
//!     ConnectionProperties, Result,
//! };
//! use log::info;
//!
//! fn main() -> Result<()> {
//!     env_logger::init();
//!
//!     let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
//!     let mut executor = LocalPool::new();
//!     let spawner = executor.spawner();
//!
//!     executor.run_until(async {
//!         let conn = Connection::connect(
//!             &addr,
//!             ConnectionProperties::default().with_default_executor(8),
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
//!         let consumer = channel_b
//!             .clone()
//!             .basic_consume(
//!                 "hello",
//!                 "my_consumer",
//!                 BasicConsumeOptions::default(),
//!                 FieldTable::default(),
//!             )
//!             .await?;
//!         let _consumer = spawner.spawn_local(async move {
//!             info!("will consume");
//!             consumer
//!                 .for_each(move |delivery| {
//!                     let delivery = delivery.expect("error caught in in consumer");
//!                     channel_b
//!                         .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
//!                         .map(|_| ())
//!                 })
//!                 .await
//!         });
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

pub use amq_protocol::{
    auth,
    protocol::{self, BasicProperties},
    tcp, types, uri,
};

pub use channel::{options, Channel};
pub use channel_status::{ChannelState, ChannelStatus};
pub use close_on_drop::CloseOnDrop;
pub use configuration::Configuration;
pub use connection::{Connect, Connection};
pub use connection_properties::ConnectionProperties;
pub use connection_status::{ConnectionState, ConnectionStatus};
pub use consumer::{Consumer, ConsumerDelegate, ConsumerIterator};
pub use error::{Error, Result};
pub use exchange::ExchangeKind;
pub use queue::Queue;

pub mod executor;
pub mod message;
pub mod publisher_confirm;

pub type Promise<T> = pinky_swear::PinkySwear<Result<T>>;
pub type PromiseChain<T> = pinky_swear::PinkySwear<Result<T>, Result<()>>;
pub type CloseOnDropPromise<T> = PromiseChain<CloseOnDrop<T>>;
pub type ConfirmationPromise<T> =
    pinky_swear::PinkySwear<Result<T>, Result<publisher_confirm::Confirmation>>;

type ConfirmationBroadcaster =
    pinky_swear::PinkyBroadcaster<Result<publisher_confirm::Confirmation>>;
type PromiseResolver<T> = pinky_swear::Pinky<Result<T>>;

mod acknowledgement;
mod buffer;
mod channel;
mod channel_status;
mod channels;
mod close_on_drop;
mod configuration;
mod connection;
mod connection_properties;
mod connection_status;
mod consumer;
mod error;
mod error_handler;
mod exchange;
mod frames;
mod heartbeat;
mod id_sequence;
mod internal_rpc;
mod io_loop;
mod parsing;
mod queue;
mod queues;
mod returned_messages;
mod thread;
mod waker;
