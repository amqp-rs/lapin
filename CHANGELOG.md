### 1.6.6 (2020-12-25)

#### Bug Fixes

* Fix an issue with automatic connection close on drop

### 1.6.5 (2020-12-23)

#### Bug Fixes

* Treat close and error as success for consumers cancel as it forcibly implies cancelation

### 1.6.4 (2020-12-23)

#### Bug Fixes

* Cleanup basic cancel handling
* Fix undocumented rabbitmq behaviour on channel close with expected replies

### 1.6.3 (2020-12-23)

#### Bug Fixes

* Better handling of automatic consumer cancelation when connection is closing

### 1.6.2 (2020-12-23)

#### Bug Fixes

* Fix a potential race condition when automatically canceling consumer on drop

### 1.6.1 (2020-11-28)

#### Bug Fixes

* Flush more often onto socket

### 1.6.0 (2020-11-21)

#### Features

* Introduce `Connection::topology` to get a serializable representation of the declared queues and consumers
* Introduce `Connection::restore` to redeclare the things listed in a Topology
* Introduce `Delivery::acker` as a simpler way to ack/nack/reject deliveries
* `Delivery` can be dereferenced as `Acker` to directly be able to ack/nack/reject
* `BasicGetMessage` and `BasicReturnMessage` can be dereferenced as `Delivery`

### 1.5.0 (2020-10-31)

#### Misc

* Update to amq-protocol 6.0.1

### 1.4.3 (2020-10-26)

#### Misc

* Port examples to async-global-executor

### 1.4.2 (2020-10-16)

#### Misc

* Update dependencies

### 1.4.1 (2020-10-06)

#### Bug Fixes

* When converting Consumer to an Iterator, don't cancel it until the Iterator is gone

### 1.4.0 (2020-10-04)

#### Bug Fixes

* When dropping a Consumer, cancel it if it has no delegates

### 1.3.1 (2020-10-04)

#### Bug Fixes

* Fix potential leak due to consumer clone

### 1.3.0 (2020-10-04)

#### Bug Fixes

* When dropping a Channel, wait for all its consumers to be canceled before closing it

#### Misc

* Use proper `non_exhaustive` annotation for error enum

### 1.2.8 (2020-09-25)

#### Misc

* Update async-task

### 1.2.7 (2020-09-18)

#### Bug Fixes

* Make frame ordering more robust

### 1.2.6 (2020-09-18)

#### Bug Fixes

* Fix a bug in the frame ordering when doing lots of publishing

### 1.2.5 (2020-09-18)

#### Misc

* Update pinky-swear

### 1.2.4 (2020-09-17)

#### Bug Fixes

* Fix a memory leak in publishers confirm

### 1.2.3 (2020-09-02)

#### Bug Fixes

* Fix an issue when redeclaring a queue

### 1.2.2 (2020-08-21)

#### Misc

* Improve logging on connection error

### 1.2.1 (2020-06-26)

#### Features

* Introduce bastion-amqp to use the bastion executor instead of the default one

#### Bug Fixes

* Properly wait for plain stream to be connected on windows

### 1.2.0 (2020-06-25)

#### Misc

* Update amq-protocol

### 1.1.3 (2020-06-23)

#### Misc

* Update dependencies

### 1.1.2 (2020-06-18)

#### Features

* Add `BasicReturnMessage::error`

### 1.1.1 (2020-06-17)

#### Bug Fixes

* Properly handle recycling channels ids when we reached the channel limit
* Fix receiving returned messages from the AMQP server

### 1.1.0 (2020-06-09)

#### Features

* Make codegen optional

### 1.0.2 (2020-06-04)

#### Bug Fixes

* update amq-protocol

### 1.0.1 (2020-06-02)

#### Bug Fixes

* `DefaultExecutor` cleanups

### 1.0.0 (2020-05-27)

#### Features

* Introduce tokio-amqp to use the tokio executor instead of the default one
* Introduce async-amqp to use the async-std executor instead of the default one
* Introduce lapinou to use the smol executor instead of the default one
* New default Reactor
* Reactor is now configurable
* Using self-signed certificates is now easie with `connect_with_config`
* `Channel` gets closed once the last reference is dropped
* `Connection` gets closed once the last reference and the last `Channel` are dropped
* `Consumer::tag` gives you access to the consumer's tag

#### Breaking changes

* The auth mechanism is now configured using a query parameter in the AMQPUri
* The executor trait is now made to handle futures. (e.g. `Box::pin(async move {})`)
* ConsumerDelegate is now built with futures.
* The number of threads used by the default executor is now configured using `ConnectionProperties::with_default_executor`
* `Connection` no longer implements `Clone`
* `Connection::connect` has been reworked to properly handle mid handshake TLS streams
* `CloseOnDrop` is now gone
* `DeliveryResult` now also carries the corresponding `Channel`

#### Bug Fixes

* Follow the specifications in being stricter about receiving invalid frames
* Don't consider interrupted IO signals as failures
* Threads are correctly cleaned up

#### Misc

* Better Debug implementations
* Internals cleanup
* Use vectored io when applicable

### 0.39.7 (2020-04-17)

#### Bug Fixes

* Error handling fixes

### 0.39.6 (2020-04-17)

#### Bug Fixes

* IoLoop error handling fixes

### 0.39.5 (2020-04-17)

#### Bug Fixes

* Fix a hang with rustls flush

### 0.39.4 (2020-04-17)

#### Bug Fixes

* Fix some edge cases issues with the internal buffer

### 0.39.3 (2020-04-17)

#### Bug Fixes

* Fix some issues with openssl
* Flush socket for rustls

### 0.39.2 (2020-04-16)

#### Bug Fixes

* Work around a bug in mio where we sometimes wouldn't receive a readable notification for the socket

### 0.39.1 (2020-04-14)

#### Bug Fixes

* Fix overflow with high heartbeats
* Properly report connection errors

### 0.39.0 (2020-04-08)

#### Breaking changes

* `Connection::on_error` and `Consumer::set_delegate` no longer require a `Box`
* `Connection::on_error` now accept a `FnMut`
* Update to pinky-swear 4.0.0

### 0.38.4 (2020-04-07)

#### Bug Fixes

* Don't queue extraneous heartbeats if we cannot write to socket

### 0.38.4 (2020-04-07)

#### Bug Fixes

* Refuse invalid incoming frames
* Don't send heartbeats unless necessary

#### Misc

* Drop the heartbeat thread

### 0.38.3 (2020-04-06)

#### Bug Fixes

* Fix the way we schedule heartbeats to be sent

### 0.38.2 (2020-04-06)

#### Bug Fixes

* Properly put the Conneciton in Error state when we get closed with protocol error

### 0.38.1 (2020-04-06)

#### Bug Fixes

* Don't panic when CloseOnDrop tries to close an already closed Connection

#### Misc

* Use vectored IO when appropriate when writing to socket

### 0.38.0 (2020-04-05)

#### Bug Fixes

* Wait until a frame has been fully sent before notifying sender
* Properly handle Basic.Return when channel is not in confirm mode
* Fix handling of publisher-confirms ack with returned message
* Fix handling of publisher-confirms nack without returned message

#### Breaking changes

* `Confirmation::Nack` now carries an `Option<Box<BasicReturnMessage>>`
* `Confirmation::Ack` now carries an `Option<Box<BasicReturnMessage>>`

### 0.37.1 (2020-04-04)

#### Bug Fixes

* Prevent a issue when hitting EWOULDBLOCK mid-frame with no other frame queued

### 0.37.0 (2020-04-03)

#### Features

* `Connection` and `Channel` can be moved out of `CloseOnDrop` using `into_inner()`
* We now add a marker to promises when trace logging is enabled to make debugging easier

#### Breaking changes

* Update to pinky-swear 3.0.0 (properly handle chaining result promises)
* `Confirmation` can no longer hold an error
* `PublisherConfirm` now returns a proper `Result<Confirmation>`
* Connecting now returns a `CloseOnDrop<Connection>`
* Creating a channel now returns a `CloseOnDrop<Channel>`
* `PinkySwear` is now hidden from the public API

### 0.36.2 (2020-04-02)

#### Features

* Add a new CloseOnDrop wrapper for automatically closing Connection or Channel when dropped.

#### Bug Fixes

* Properly send errors to publisher confirms awaiters on channel close

### 0.36.1 (2020-04-02)

#### Bug Fixes

* Properly forward errors to publisher confirms awaiters

### 0.36.0 (2020-04-02)

#### Bug Fixes

* Track the frame-sending promise before the response one for syncrhonous methods

#### Breaking changes

* The bugfix induces some changes in the return values of some method. Shouldn't change anything.

### 0.35.2 (2020-04-02)

#### Bug Fixes

* Properly report error in one internal place

### 0.35.1 (2020-04-02)

#### Bug Fixes

* Update pinky-swear to 2.1.0 to avoid any sync op on main thread

### 0.35.0 (2020-04-01)

#### Breaking changes

* Update to pinky-swear 2.0.0 (shoudln't change much)
* Publisher Confirms integration has been reworked:
  * `basic_publish` now gives you a `PinkySwear<Result<PublisherConfirm>>`
  * If you didn't enable publisher confirms using `confirm_select`, you can ignore it
  * If you drop the `PublisherConfirm` it will behave as before and you'll need to call `wait_for_confirms`
  * You can use the ` PublisherConfirm` to wait for this particular `Confirmation` (`Ack`/`Nack(BasicReturnMEssage)`)

#### Bug Fixes

* Detect errors in some edge scenarii

### 0.34.1 (2020-04-01)

#### Bug Fixes

* Fix a bug when receiving basic-return before basic-nack

### 0.34.0 (2020-03-24)

#### Breaking changes

* The `Connect` trait has been simplified
* The `futures` feature doesn't exist anymore (always enabled)
* The `lapin-futures` (0.1 futures compat) has been dropped

#### Misc

* Examples ported to async/await

### 0.33.2 (2020-03-23)

#### Bug Fixes

* Fix nom dependency

### 0.33.1 (2020-03-18)

#### Bug Fixes

* Fix hang on macos

### 0.33.0 (2020-03-12)

#### Breaking changes

* Some more Error reworks
* Port to amq-protocol 5.0 and mio 0.7
* Deprecate lapin-futures

### 0.32.5 (2020-03-09)

#### Bug Fixes

* Prevent lapin-futures from eating memory when eagerly polled

### 0.32.4 (2020-03-09)

#### Bug Fixes

* Followup to 0.32.3 for consumers in lapin-futures

### 0.32.3 (2020-03-09)

#### Bug Fixes

* Fix waking up context when a future gets sent to another one

### 0.32.2 (2020-03-05)

#### Bug Fixes

* Fix race condition in `Channel::close`

### 0.32.1 (2020-03-03)

#### Bug Fixes

* Fix Channel and Connection status in some cases

### 0.32.0 (2020-02-27)

#### Bug Fixes

* Properly handle critical error when reading from socket on first connection

#### Breaking changes

* `Error::ConnectionRefused` is replaced by the proper underlying ProtocolError or IOError

#### Features

* `DefaultExecutor::default()` is now public

### 0.31.0 (2020-02-25)

#### Bug Fixes

* Receiving consumer or basic get deliveries on queue we didn't declare now works properly

#### Breaking changes

* `Channel::basic_consume` now takes an `&str` instead of a `&Queue` as its first parameter

#### Features

* `impl From<_> for AMQPValue`

### 0.30.1 (2020-02-25)

#### Features

* New rustls-native-certs feature

### 0.30.0 (2020-02-25)

#### Breaking changes

* Error enum has been cleaned up
* ErrorHandler now takes the Error as a param

#### Bug Fixes

* Better error handling and forwarding

### 0.29.2 (2020-02-25)

#### Bug Fixes

* Fix PartialEq for Error

### 0.29.1 (2020-02-24)

#### Bug Fixes

* Ensure we properly write everything when we need to split contents

### 0.29.0 (2020-02-24)

#### Breaking changes

* Switch the return types to `pinky-swear` (the API remains mostly the same)

#### Features

* Error is now Clonable

#### Bug Fixes

* Better error handling and bubbling in lots of cases
* Properly handle soft errors

### 0.28.4 (2019-12-03)

#### Bug Fixes

* Fix some error handling in consumers

### 0.28.3 (2019-11-21)

#### Features

* Export ConsumerIterator

### 0.28.2 (2019-11-07)

#### Features

* Update dependencies

### 0.28.1 (20191023)

#### Bug Fixes

* Fix some race conditions in the publisher confirms implementation

### 0.28.0 (20190927)

#### Breaking changes

* `Channel::exchange_declare` now takes an `ExchangeKind` parameter instead of an `&str`

### 0.27.2 (2019-11-07)

#### Features

* Update dependencies

### 0.27.1 (2019-09-26)

#### Bug Fixes

* Warn on unused `Confirmation`
* Avoid Mutex around consumer delegate to enable proper multithreading

### 0.27.0 (2019-09-25)

#### Breaking changes

* Updated amq-protocol to 3.0.0
* `Channel::connection_[,un}blocked` is now `Connection::{,un}block`
* `failure` as been replaced with `std::error::Error` usage
* `Confirmation::as_error` has been removed
* Consumers API has been cleaned up, everything is now a `DeliveryResult`
* `IoLoop::run` is now `IoLoop::start`

#### Features

* Add support for `update_secret` for oauth2 authentication module
* Add support for TLS "identity" (client certificate)
* Consumer can now be used as an `Iterator<Item = Delivery>`
* `Consumer::set_delegate` now accepts a closure parameter
* Add `lapin::Result`

### 0.26.11 (2019-09-17)

#### Bug Fixes

* Update amq-protocol to fix amqps handling

### 0.26.10 (2019-09-11)

#### Bug Fixes

* Fix error handling during early connection stage
* Properly forward errors to consumer delegates

### 0.26.9 (2019-08-26)

#### Bug Fixes

* `IoLoop` fixes under heavy loads

### 0.26.8 (2019-08-26)

#### Bug Fixes

* `IoLoop` fixes under heavy loads

### 0.26.7 (2019-08-23)

#### Bug Fixes

* Make `Connection::connector` and `IoLoop:run` public

### 0.26.6 (2019-08-15)

#### Bug Fixes

* Better handle multiple channel publishing under heavy load

### 0.26.5 (2019-08-14)

#### Bug Fixes

* Fix retrying of `basic_publish` frames

### 0.26.4 (2019-08-14)

#### Bug Fixes

* Rework how `basic_publish` is handled internally to ensure concurrent usages work as expected

### 0.26.3 (2019-08-12)

#### Bug Fixes

* Do not hang on tasks that require an answer in case of channel error

### 0.26.2 (2019-08-12)

#### Bug Fixes

* Fixes some frames ordering when using concurrent `basic_publish` under heavy load

### 0.26.1 (2019-08-09)

#### Bug Fixes

* Properly broadcast channel error to all pending tasks/futures

### 0.26.0 (2019-08-08)

#### Bug Fixes

* Fix unblocking connection
* Properly broadcast connection error to all pending tasks/futures

#### Breaking changes

* Unused IoLoopError has been dropped

### 0.25.0 (2019-07-12)

#### Bug Fixes

* Consumer streams now properly forward connection errors

#### Breaking changes

* lapin's consumer stream now returns a Result

#### Features

* `ConsumerDelegate` now has a `on_error` hook

### 0.24.1 (2019-07-11)

#### Bug Fixes

* Properly handle network disconnections on OSX

### 0.24.0 (2019-07-04)

#### Bug Fixes

* `Connection::close` no longer hangs
* `ConsumerDelegate` no longer requires `fmt::Debug`

#### Breaking changes

* `ConsumerDelegate` methods have been renamed for clarity and only `on_new_delivery` is now mandatory

### 0.23.0 (2019-06-21)

#### Breaking changes

* `lapin-async` as been renamed to `lapin`
* lapin: Instead of passing a `Box<dyn ConsumerSubscriber>` as a parameter to `basic_consume`, you must now call
  `set_delegate(Box<dyn ConsumerDelegate>)` on the returned `Consumer`

#### Features

* `lapin` has experimental support for `futures-0.3` + `std::future::Future` through its `futures` feature

### 0.22.0 (2019-06-20)

#### Features

* you can now select the TLS implementation used for amqps or disable amqps support

#### Bug Fixes

* vhosts are properly handled again
* we now properly wait for the return message when last ack is a nack for publishers confirm

#### Breaking changes

* `wait_for_confirms()` is now async (needs to be awaited)

### 0.21.3 (2019-06-18)

#### Bug Fixes

* More work around connection failures, properly report those as errors
* Add a way to register a connection error handler

### 0.21.2 (2019-06-17)

#### Bug Fixes

* Properly handle connection failures

### 0.21.1 (2019-06-16)

#### Features

* **async**
  * `Connection::run` to keep the program running when there is nothing left to downgraded but consume new messages

#### Bug Fixes

* `io_loop` correctly exists once connection is no longer connected

### 0.21.0 (2019-06-14)

#### Breaking changes

* Some internal methods are no longer public (channel and connection handling)
* Rework how we close channels and connection

### 0.20.0 (2019-06-14)

#### Breaking changes

* Drop duplicate Credentials param from connect, use the credentials from the AMQPUri.

### 0.19.0 (2019-06-14)

#### Features

* All of AMQP methods and auth mechanisms are now supported

#### Bug Fixes

* Better consumers handling
* Misc code cleanup and modernization
* AMQP is now fully supported, no more crahs on unexpected frames

#### Breaking changes

* Method options are now generated. Hardcoded fields from AMQP omitted. Options are shared between async and futures
* The way we handle `publisher_confirm` has changed. You now need to call `confirm_select` explicitly, and then
  `wait_for_confirms` to wait for all pending confirmations
* **async**
  * Methods are now on the `Channel` object which is now returned instead of `channel_id` by `create_channel`
  * Methods are now generated from protocol specifications
  * Methods return a Confirmation that can be awaited
  * ShortString and LongString are now concrete types (!= String), which can be created from &str or String using `into()`
  * Connection::connect has been rewritten
* **futures**
  * Port to the new lapin-async
  * Client::connect has been rewritten

### 0.18.0 (2019-03-03)

#### Bug Fixes

* Better `delivery_tag` handling
* Adapt our behaviour wrt ack/nack to be specifications-compliant
* We now pass several additional information to the server when connecting, such as capabilities

#### Breaking changes

* Connect now takes an additional `ConnectionProperties` for better configuration

#### Features

* Better logging when channel gets closed by server
* Support receiving BasicCancel from the server

### 0.17.0 (2019-02-15)

#### Bug Fixes

* Drop prefetched messages when specific arguments are passed to `basic_{,n}ack` or `basic_cancel`

#### Housekeeping

* Drop sasl dependency, avoiding linkage to LGPL-3 licensed code

### 0.16.0 (2019-02-01)

#### Housekeeping

* Switch to edition 2018
* Switch to `parking_lot` Mutex

#### Breaking changes

* **futures**
  * Drop now unused mutex poisoning error

### 0.15.0 (2018-12-05)

#### Housekeeping

* Update `amq-protocol`

#### Breaking Changes

* **async:**
  * Introduce a new `Error` type, replacing occurrences of `io::Error` in public APIs ([#145](https://github.com/CleverCloud/lapin/pull/147))
* **futures:**
  * Introduce a new `Error` type, replacing occurrences of `io::Error` in public APIs ([#145](https://github.com/CleverCloud/lapin/pull/145))

### 0.14.1 (2018-11-16)

#### Housekeeping

* Update `env_logger`
* Drop unused `build.rs` from async

#### Bug Fixes

* Fix heartbeat interval

### 0.14.0 (2018-10-17)

#### Housekeeping

* Update amq-protocol dependency
* Reexport `amq_protocol::uri`

### 0.13.0 (2018-07-09)

#### Features

* **futures:**
  * `basic_ack` and `basic_nack` API now support passing the `multiple` flag
  * Port to the new `tokio-codec` crate
  * The object returned by `queue_declare` now holds the messages count and the consumers count too

#### Bug Fixes

* Fully rework how consumers are handled internally, should be way more robust now
* Heartbeats are now preemptive and are successfully sent even under heavy load

#### Breaking Changes

* Port to `nom` 4
* **async:** some fields got their visibility downgraded to private as part of the consumers rework
* **futures:**
  * We now use `impl trait` and thus require rust 1.26.0 or greater
  * `basic_publish` payload is now a `Vec<u8>`

### 0.12.0 (2018-06-05)

#### Features

* Implement `channel_close_ok`
* Slightly rework consumers internal handling
* **futures:**
  * Allow cancelling the Heartbeat future
  * Implement flow methods

#### Bug Fixes

* Fix bad expectation for empty payloads
* Fix heartbeat when configured value is 0
* Fix channel overflow when `channel_max` is low
* **futures:**
  * Ensure tasks aren't dropped when we hit `Async::NotReady` but queued for re-poll instead
  * Correctly handle mutex poisoning
  * Use generated consumer tag and queue name when an empty one is provided
  * Fix `Sink` implementation on `AMQPTransport`

#### Breaking Changes

* **futures:**
  * Port to `tokio`
  * Update to `tokio-timer` 0.2
  * `queue_declare` now return a `Queue` object
  * `basic_consume` now expects a `Queue` object to ensure you've called `queue_declare first`

### 0.11.1 (2018-04-12)

#### Bug Fixes

* **futures:** Get back to `tokio-timer` 0.1

### 0.11.0 (2018-04-08)

#### Features

* implement `basic_qos`
* **futures:**
  * Implement `basic_nack`
  * Implement `queue_unbind`
  * Mark all futures as `Send` to ease `tokio` integration

#### Bug Fixes

* **futures:** Get back to `tokio-timer` 0.1

#### Breaking Changes

* `Message` is now `Delivery`, differentiate from `BasicGetMessage`
* **futures:**
  * Port to `tokio-timer` 0.2
  * Prefer `handle.spawn` to `thread::new` for the heartbeat

### 0.10.0 (2017-07-13)

#### Bug Fixes

* Rework how the futures API is handled internally
* Rework client-server parameters negotiation at connection

#### Breaking Changes

* **futures:**
  * `create_confirm_channel` now take a `ConfirmSelectOptions`
  * Run heartbeat in a separated thread (delegate thread creation to user)
  * Return a heartbeat creation closure alongside the client

### 0.9.0 (2017-06-19)

#### Features

* Implement `access` methods
* **async:**
  * Make errors more specific
  * Do the `frame_max` negotiation with the server
* **futures:**
  * Implement missing exchange methods

#### Breaking Changes

* **futures:** Rework the `basic_publish` API

### 0.8.2 (2017-05-04)

#### Bug Fixes

* Update `amq-protocol`

### 0.8.1 (2017-05-04)

#### Bug Fixes

* Better error handling

### 0.8.0 (2017-04-13)

#### Features

* **futures:** Implement `Channel::close`

#### Bug Fixes

* Polling improvements
- Better error handling

### 0.7.0 (2017-04-10)

#### Features

* Implement `exchange_declare`
* **futures:**
  * Implement `queue_bind`
  * Implement `queue_delete`

### 0.6.0 (2017-03-30)

#### Features

* Allow choosing the vhost

### 0.5.0 (2017-03-28)

#### Bug Fixes

* Update `sasl` to 0.4 (removes the `openssl` dependency)

### 0.4.0 (2017-03-28)

#### Features

* Implement `confirm_select`
* **async:**
  * Add support for BasicProperties
  * Implement `receive_basic_ack`
  * Implement `receive_basic_nack`
* **futures:**
  * Implement confirm channel
  * Implement heartbeat

### 0.3.0 (2017-03-25)

#### Features

* Chunking of message bodies

#### Breaking Changes

* **futures:** Add options for API methods missing them

### 0.2.0 (2017-03-20)

#### Features

* Initial release

