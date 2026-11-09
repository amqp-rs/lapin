# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build and type-check (--all-features requires system deps for openssl; omit locally if Perl unavailable)
cargo check --all --bins --examples --tests --all-features

# Run all tests (requires a running RabbitMQ instance)
cargo test

# Run a single test file
cargo test --test tokio           # tokio integration tests
cargo test --test smol            # smol integration tests
cargo test --test runtime_isolation --features=tokio

# Lint
cargo clippy --all-features -- -W clippy::all

# Format check
cargo fmt --all -- --check

# Format (apply)
cargo fmt --all

# Docs
RUSTDOCFLAGS=-D warnings cargo doc --no-deps --document-private-items --all-features

# Regenerate protocol code (after changing templates/ or lapin.json)
./regen-code.sh
```

Tests require RabbitMQ running locally (default port 5672). The CI uses a Docker `rabbitmq:latest` service.

## Architecture

Lapin is an async AMQP 0.9.1 client. The core abstraction is a `Connection` (one TCP socket) that multiplexes many lightweight `Channel`s. All I/O runs on a single background thread (the IO loop); user code interacts with it via lock-free channels.

### Key layers

**Connection & Channel (`connection.rs`, `channel.rs`)**
- `Connection::connect()` / `Connection::connect_with_runtime()` are the entry points.
- `connection.create_channel()` creates logical channels on the same socket.
- `channel.rs` (~44 KB, hand-written) implements every AMQP operation (basic_publish, basic_consume, queue_declare, …). It delegates the heavy lifting to the generated layer.
- `src/generated/channel.rs` (~73 KB, **do not edit by hand**) is produced by the codegen system and contains all option structs and low-level method implementations.

**IO Loop (`io_loop.rs`)**
- Spawned once per connection as a background thread via `runtime.rs`.
- Owns the socket, read/write buffers, and the heartbeat timer.
- Successfully processed OpenOk records the transport's completed handshake before checking the shared lifecycle again. An immediate public close keeps the IO loop alive while its close sender is still pending.
- An EOF that does not fail the transport at once, during a handshake or a transport switch, stops reading: pending writes are waited for, EOF is not read again.
- Implements automatic reconnection with exponential backoff (`backon` crate).
- State machine: `Initial → Connected → Stop`.

**Internal RPC (`internal_rpc.rs`)**
- `InternalRPCHandle` (cloneable `flume` sender) lets channels submit commands (Ack, Nack, Reject, CreateChannel, CloseChannel, …) to the IO loop without shared mutable state.
- `InternalRPC` (receiver side) processes the command queue inside the IO loop.
- Stopping the dispatcher detaches its shared sender, rejects later calls, and drains any send that already cloned the sender after the stop sentinel.

**Frame handling (`frames.rs`, `parsing.rs`)**
- `Frames` manages the outbound frame queue and tracks expected replies.
- Paused publishes and subsequent methods are deferred per channel; `Channel.Flow` and `Channel.FlowOk` bypass that queue so flow-control handshakes can complete.
- `ExpectedReply` associates an outgoing frame with a promise resolver so callers can `await` the server response.
- Frame serialization/deserialization is done by the `amq-protocol` crate.
- Generated methods check their short string, long string and field table arguments (`Sendable` in `channel.rs`) before their start hook runs or anything is queued, and a publish serializes its method and header aside: a value the IO loop could not serialize, such as a short string holding NUL, fails only that call with `SerialisationError`, without local state changes or a publisher confirm. `Connection::close` checks its reply text before the connection starts closing. Handshake steps are not checked: a failure there still fails the connection.

**Promise system (`promise.rs`)**
- Thin async primitive used instead of `oneshot` channels for RPC-style request/response throughout the library.
- `Shared<T>` holds the result behind a `Mutex<Option<Result<T>>>` (for ownership transfer on take) and a `Listener` (notify side only) for waking the waiting task.
- `Promise<T>` holds its own `Listener` (cloned from `Shared` at construction, sharing the same `Arc<Event>`) for the arm/poll side.
- `poll` uses the arm-then-recheck loop to close the lost-wakeup race window; the resolver drops the data lock before calling `notify()`.

**Publisher confirms (`acknowledgement.rs`)**
- `wait_for_confirms` waits for every publish confirmation pending when it is called; concurrent waiters do not consume each other. A channel failure or recovery reset that rejects confirms no waiter covered is kept and fails the next `wait_for_confirms` once.

**Listener (`listener.rs`)**
- `Listener` wraps `event_listener::{Event, EventListener}` and is used by `Promise`, `Consumer`'s stream impl, and `Notifier`.
- Usage pattern: call `arm()` before checking the guarded condition, `disarm()` if it is met, otherwise `poll()` to register the waker; `notify()` wakes all clones sharing the same `Event`.

**Events (`events.rs`)**
- The connection-side registry owns event senders. Listener streams remove their registration on drop and end after all event producers are dropped.
- A listener created while the connection is open first receives `Event::Connected`. Channel error events preserve that snapshot while the connection itself remains connected.
- Event delivery and sender destruction happen outside the registry lock so listener wakers can register or drop other listeners. Every listener still sees events in emission order: an emitter that finds another one delivering leaves its event to it, which also lets a listener waker emit events.

**Topology & recovery (`topology.rs`, `channel_recovery_context.rs`)**
- `Topology` tracks declared exchanges, queues, and bindings so the IO loop can replay them after a reconnect.
- `ConnectionProperties::enable_auto_recover()` enables this behavior.

### Code generation

Templates live in `templates/channel.rs` (Handlebars) and `templates/lapin.json` (extra metadata). Running `./regen-code.sh` sets `LAPIN_CODEGEN_DIR=src/generated`, invokes `cargo build --features=codegen-internal`, and then formats the output. The generated file is committed; `build.rs` only regenerates it when the `codegen-internal` feature is active.

When the AMQP spec or method signatures need to change, edit `templates/` and re-run `./regen-code.sh`, then commit both.

### Feature flags

| Category | Flags |
|----------|-------|
| Runtime (pick one) | `tokio` (default), `smol`, `async-global-executor` |
| TLS | `rustls` (default), `native-tls`, `openssl`, `rustls-platform-verifier`, `rustls-native-certs`, `rustls-webpki-roots-certs` |
| Rustls crypto | `rustls--aws_lc_rs` (default), `rustls--ring` (more portable) |
| DNS | `hickory-dns` |
| Codegen | `codegen` (user-facing), `codegen-internal` (build.rs only) |

MSRV is **1.89.0** (Rust 2024 edition).
