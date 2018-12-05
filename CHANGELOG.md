### Unreleased

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
* Fix channel overflox when `channel_max` is low
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

