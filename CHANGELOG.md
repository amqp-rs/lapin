### 0.27.0 (XXX)

#### Breaking changes

* Updated amq-protocol to 3.0.0
* `Channel::connection_[,un}blocked` is now `Connection::{,un}block`
* `failure` as been replaced with `std::error::Error` usage
* `Confirmation::as_error` has been removed

#### Features

* Add support for `update_secret` for oauth2 authentication module
* Add support for TLS "identity" (client certificate)

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
* The way we handle `publisher_confirm` has changed. You now need to call `confirm_select` explicitely, and then
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

* Drop prefetched messages when speicific arguments are passed to `basic_{,n}ack` or `basic_cancel`

#### Housekeeping

* Drop sasl dependency, avoiding likage to LGPL-3 licensed code

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
  * Introduce a new `Error` type, replacing occurences of `io::Error` in public APIs ([#145](https://github.com/sozu-proxy/lapin/pull/147))
* **futures:**
  * Introduce a new `Error` type, replacing occurences of `io::Error` in public APIs ([#145](https://github.com/sozu-proxy/lapin/pull/145))

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
* Heartbeats are now preemptive and are sucessfully sent even under heavy load

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
* Rework client-server parameters negociation at connection

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
  * Do the `frame_max` negociation with the server
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

* Allow chosing the vhost

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

