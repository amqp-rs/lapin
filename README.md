[![Build Status](https://travis-ci.org/sozu-proxy/lapin.svg?branch=master)](https://travis-ci.org/sozu-proxy/lapin)
[![Coverage Status](https://coveralls.io/repos/github/sozu-proxy/lapin/badge.svg?branch=master)](https://coveralls.io/github/sozu-proxy/lapin?branch=master)
[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Dependency Status](https://deps.rs/repo/github/sozu-proxy/lapin/status.svg)](https://deps.rs/repo/github/sozu-proxy/lapin)

# lapin, a Rust AMQP client library

![](logo.jpg)

this project is separated into two crates. See the READMEs in the subfolder for each library.

It follows the AMQP 0.9.1 specifications, targetting especially RabbitMQ. As this is a young project,
only part of the specification is implemented now.

What you can do:

- connect to a server, close the connection
- create and close channels
- using all the basic methods. That means you can publish messages, make a consumer, ack or reject messages
- authentication method is only PLAIN for now

## lapin-futures

[![Crates.io Version](https://img.shields.io/crates/v/lapin-futures.svg)](https://crates.io/crates/lapin-futures)

a library with a futures based API, that you can use with tokio-core or futures-cpupool.

This is the recommended way to use lapin as an AMQP client.

lapin-futures is available on [crates.io](https://crates.io/crates/lapin-futures) and can be included in your Cargo enabled project like this:

```toml
[dependencies]
lapin-futures = "^0.21"
```

Then include it in your code like this:

```rust
use lapin_futures;
```

## lapin-async

[![Crates.io Version](https://img.shields.io/crates/v/lapin-async.svg)](https://crates.io/crates/lapin-async)

A low level library meant for usage in an event loop like one you'd build with mio.
This library assumes non blocking IO.

lapin-async is available on [crates.io](https://crates.io/crates/lapin-async) and can be included in your Cargo enabled project like this:

```toml
[dependencies]
lapin-async = "^0.21"
```

Then include it in your code like this:

```rust
use lapin_async;
```
