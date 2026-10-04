#![cfg(feature = "tokio")]

use amq_protocol::{
    frame::{AMQPFrame, gen_frame, parse_frame},
    protocol::{AMQPClass, connection},
};
use async_rs::Runtime;
use lapin::{
    Connection, ConnectionProperties, ErrorKind,
    auth::AuthProvider,
    types::{LongString, ShortString},
};
use std::{
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(5);

fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // The fake broker reads and writes with blocking calls and timeouts. On
                // Windows an accepted socket inherits the listener's non-blocking mode.
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(TIMEOUT)).unwrap();
                stream.set_write_timeout(Some(TIMEOUT)).unwrap();
                return stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "client did not retry TCP");
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    }
}

fn read_header(stream: &mut TcpStream) {
    let mut header = [0; 8];
    stream.read_exact(&mut header).unwrap();
    assert_eq!(&header, b"AMQP\0\0\x09\x01");
}

fn send_method(stream: &mut TcpStream, method: connection::AMQPMethod) {
    let frame = AMQPFrame::Method(0, AMQPClass::Connection(method));
    let bytes = gen_frame(&frame)(Vec::new().into()).unwrap().into_inner().0;
    stream.write_all(&bytes).unwrap();
}

fn read_method(stream: &mut TcpStream) -> connection::AMQPMethod {
    read_method_up_to(stream, 4096)
}

fn read_method_up_to(stream: &mut TcpStream, max_size: usize) -> connection::AMQPMethod {
    let mut header = [0; 7];
    stream.read_exact(&mut header).unwrap();
    let size = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
    assert!(size < max_size, "unexpected frame size: {size}");
    let mut bytes = vec![0; 7 + size + 1];
    bytes[..7].copy_from_slice(&header);
    stream.read_exact(&mut bytes[7..]).unwrap();
    match parse_frame(bytes.as_slice()).unwrap().1 {
        AMQPFrame::Method(0, AMQPClass::Connection(method)) => method,
        frame => panic!("unexpected frame: {frame:?}"),
    }
}

fn send_start(stream: &mut TcpStream) {
    send_method(
        stream,
        connection::AMQPMethod::Start(connection::Start {
            version_major: 0,
            version_minor: 9,
            mechanisms: "PLAIN".into(),
            locales: "en_US".into(),
            ..Default::default()
        }),
    );
}

#[tokio::test]
async fn unlimited_server_offer_keeps_buffers_bounded() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        send_start(&mut stream);
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::StartOk(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::Tune(connection::Tune {
                channel_max: 0,
                frame_max: 0,
                heartbeat: 0,
            }),
        );
        match read_method(&mut stream) {
            connection::AMQPMethod::TuneOk(tune) => assert_eq!(tune.frame_max, 16 * 1024 * 1024),
            method => panic!("unexpected method: {method:?}"),
        }
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Open(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::OpenOk(connection::OpenOk {}),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Close(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::CloseOk(connection::CloseOk {}),
        );
    });

    let connection = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, properties(0), Runtime::tokio_current()),
    )
    .await
    .expect("unlimited frame offer hung")
    .unwrap();
    assert_eq!(connection.configuration().frame_max(), 16 * 1024 * 1024);
    connection.close(200, "done".into()).await.unwrap();
    server.join().unwrap();
}

#[tokio::test]
async fn outbound_handshake_frame_grows_past_initial_buffer() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        send_start(&mut stream);
        assert!(matches!(
            read_method_up_to(&mut stream, 256 * 1024),
            connection::AMQPMethod::StartOk(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::Tune(connection::Tune {
                channel_max: 16,
                frame_max: 4096,
                heartbeat: 0,
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::TuneOk(_)
        ));
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Open(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::OpenOk(connection::OpenOk {}),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Close(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::CloseOk(connection::CloseOk {}),
        );
    });
    let options = properties(0).with_client_property("large".into(), "x".repeat(128 * 1024).into());
    let connection = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, options, Runtime::tokio_current()),
    )
    .await
    .expect("large handshake frame hung")
    .unwrap();
    connection.close(200, "done".into()).await.unwrap();
    server.join().unwrap();
}

/// Answers a challenge only if it arrived intact.
struct ExpectChallenge(Vec<u8>);

impl AuthProvider for ExpectChallenge {
    fn mechanism(&self) -> ShortString {
        "PLAIN".into()
    }

    fn auth_starter(&self) -> Result<LongString, String> {
        Ok("credentials".into())
    }

    fn continue_auth(&self, challenge: LongString) -> Result<LongString, String> {
        if challenge.as_bytes() == self.0 {
            Ok("response".into())
        } else {
            Err("the challenge was corrupted".into())
        }
    }
}

#[tokio::test]
async fn inbound_handshake_frame_grows_past_initial_buffer() {
    const INITIAL_BUFFER_SIZE: usize = 64 * 1024;
    let challenge = (0..100 * 1024).map(|i| (i % 251) as u8).collect::<Vec<_>>();
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let expected = challenge.clone();
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        // Once this Start is consumed, the receive buffer's read position is
        // close to its end: the Secure that follows wraps around before the
        // buffer grows to hold it.
        let start = |padding: usize| {
            let mut server_properties = lapin::types::FieldTable::default();
            server_properties.insert(
                "padding".into(),
                lapin::types::AMQPValue::LongString(vec![b'p'; padding].into()),
            );
            AMQPFrame::Method(
                0,
                AMQPClass::Connection(connection::AMQPMethod::Start(connection::Start {
                    version_major: 0,
                    version_minor: 9,
                    mechanisms: "PLAIN".into(),
                    locales: "en_US".into(),
                    server_properties,
                })),
            )
        };
        let serialize =
            |frame: &AMQPFrame| gen_frame(frame)(Vec::new().into()).unwrap().into_inner().0;
        let unpadded = serialize(&start(0)).len();
        let start = serialize(&start(INITIAL_BUFFER_SIZE - 100 - unpadded));
        assert_eq!(start.len(), INITIAL_BUFFER_SIZE - 100);
        stream.write_all(&start).unwrap();
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::StartOk(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::Secure(connection::Secure {
                challenge: challenge.into(),
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::SecureOk(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::Tune(connection::Tune {
                channel_max: 16,
                frame_max: 4096,
                heartbeat: 0,
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::TuneOk(_)
        ));
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Open(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::OpenOk(connection::OpenOk {}),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Close(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::CloseOk(connection::CloseOk {}),
        );
    });
    let options = properties(0).with_auth_provider(ExpectChallenge(expected));
    let connection = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, options, Runtime::tokio_current()),
    )
    .await
    .expect("large inbound frame hung")
    .unwrap();
    connection.close(200, "done".into()).await.unwrap();
    tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
}

#[cfg(any(feature = "rustls", feature = "native-tls", feature = "openssl"))]
#[tokio::test]
async fn uri_connection_timeout_bounds_the_dial() {
    let listener = listener();
    let uri = format!(
        "amqps://{}/?connection_timeout=200",
        listener.local_addr().unwrap()
    );
    let (done, wait_done) = std::sync::mpsc::channel::<()>();
    // A peer that accepts the TCP connection but never answers the TLS
    // handshake keeps the dial pending.
    let server = thread::spawn(move || {
        let _stream = accept(&listener);
        let _ = wait_done.recv_timeout(TIMEOUT);
    });
    let started = std::time::Instant::now();
    let error = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, properties(0), Runtime::tokio_current()),
    )
    .await
    .expect("the dial outlived the URI's connection_timeout")
    .unwrap_err();
    assert!(
        matches!(error.kind(), ErrorKind::IOError(error) if error.kind() == std::io::ErrorKind::TimedOut),
        "unexpected error: {error}"
    );
    assert!(started.elapsed() >= Duration::from_millis(200));
    done.send(()).unwrap();
    tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn oversized_outbound_handshake_frame_fails_instead_of_retrying_forever() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        send_start(&mut stream);
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    });
    let options =
        properties(0).with_client_property("huge".into(), "x".repeat(16 * 1024 * 1024).into());
    let error = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, options, Runtime::tokio_current()),
    )
    .await
    .expect("oversized outbound frame left connect pending")
    .unwrap_err();
    assert!(matches!(error.kind(), ErrorKind::SerialisationError(_)));
    server.join().unwrap();
}

#[tokio::test]
async fn oversized_inbound_header_fails_before_payload_arrives() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        stream.write_all(&[1, 0, 0, 1, 0, 0, 1]).unwrap();
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    });
    let error = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, properties(0), Runtime::tokio_current()),
    )
    .await
    .expect("oversized inbound frame left connect pending")
    .unwrap_err();
    assert!(matches!(error.kind(), ErrorKind::ProtocolError(_)));
    server.join().unwrap();
}

#[tokio::test]
async fn explicit_close_does_not_reconnect_after_socket_failure() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        send_start(&mut stream);
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::StartOk(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::Tune(connection::Tune {
                channel_max: 16,
                frame_max: 4096,
                heartbeat: 0,
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::TuneOk(_)
        ));
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Open(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::OpenOk(connection::OpenOk {}),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Close(_)
        ));
        drop(stream);
        thread::sleep(Duration::from_millis(100));
        assert!(matches!(listener.accept(), Err(error)
            if error.kind() == std::io::ErrorKind::WouldBlock));
    });

    let connection = Connection::connect_with_runtime(
        &uri,
        properties(3).enable_auto_recover(),
        Runtime::tokio_current(),
    )
    .await
    .unwrap();
    let closed = tokio::time::timeout(TIMEOUT, connection.close(200, "done".into()))
        .await
        .expect("close did not settle");
    // The socket died before Close-Ok: the close settles with that failure.
    assert!(closed.is_err());
    // Join off the runtime: lapin must keep running while the server checks
    // that no reconnection arrives.
    tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
    assert!(!connection.status().reconnecting());
    assert!(!connection.status().connected());
}

#[tokio::test]
async fn dropping_connection_while_reconnecting_stops_retry() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let (disconnect, proceed) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        send_start(&mut stream);
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::StartOk(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::Tune(connection::Tune {
                channel_max: 16,
                frame_max: 4096,
                heartbeat: 0,
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::TuneOk(_)
        ));
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Open(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::OpenOk(connection::OpenOk {}),
        );
        proceed.recv_timeout(TIMEOUT).unwrap();
        drop(stream);
        thread::sleep(Duration::from_millis(400));
        if let Ok((mut retry, _)) = listener.accept() {
            retry
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            loop {
                match retry.read(&mut [0; 1024]) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(error) => panic!("reconnect socket survived handle drop: {error}"),
                }
            }
        }
        assert!(matches!(listener.accept(), Err(error)
            if error.kind() == std::io::ErrorKind::WouldBlock));
    });

    let options = ConnectionProperties::default()
        .enable_auto_recover()
        .configure_backoff(|backoff| {
            backoff
                .with_min_delay(Duration::from_secs(2))
                .with_max_delay(Duration::from_secs(2))
                .with_max_times(3)
        });
    let connection = Connection::connect_with_runtime(&uri, options, Runtime::tokio_current())
        .await
        .unwrap();
    let status = connection.status().clone();
    disconnect.send(()).unwrap();
    tokio::time::timeout(TIMEOUT, async {
        while !status.reconnecting() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("connection did not enter recovery");
    tokio::time::timeout(
        Duration::from_secs(1),
        tokio::task::spawn_blocking(move || connection.run()),
    )
    .await
    .expect("IO thread stayed in reconnect backoff after handle drop")
    .unwrap()
    .ok();
    tokio::time::timeout(Duration::from_millis(100), async {
        while status.reconnecting() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("dropping the connection did not end recovery");
    tokio::task::spawn_blocking(move || server.join().unwrap())
        .await
        .unwrap();
    assert!(
        status.closed(),
        "shutdown was overwritten by recovery failure"
    );
}

fn listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn properties(retries: usize) -> ConnectionProperties {
    ConnectionProperties::default().configure_backoff(|backoff| {
        backoff
            .with_min_delay(Duration::from_millis(1))
            .with_max_delay(Duration::from_millis(1))
            .with_max_times(retries)
    })
}

struct FailingAuth {
    fail_starter: bool,
}

impl AuthProvider for FailingAuth {
    fn mechanism(&self) -> ShortString {
        "PLAIN".into()
    }

    fn auth_starter(&self) -> Result<LongString, String> {
        if self.fail_starter {
            Err("starter failed".into())
        } else {
            Ok("credentials".into())
        }
    }

    fn continue_auth(&self, _: LongString) -> Result<LongString, String> {
        Err("challenge failed".into())
    }
}

#[tokio::test]
async fn authentication_provider_errors_reject_connect() {
    for fail_starter in [true, false] {
        let listener = listener();
        let uri = format!("amqp://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut stream = accept(&listener);
            read_header(&mut stream);
            send_method(
                &mut stream,
                connection::AMQPMethod::Start(connection::Start {
                    version_major: 0,
                    version_minor: 9,
                    mechanisms: "PLAIN".into(),
                    locales: "en_US".into(),
                    ..Default::default()
                }),
            );
            if !fail_starter {
                assert!(matches!(
                    read_method(&mut stream),
                    connection::AMQPMethod::StartOk(_)
                ));
                send_method(
                    &mut stream,
                    connection::AMQPMethod::Secure(connection::Secure {
                        challenge: "challenge".into(),
                    }),
                );
            }
            assert_eq!(stream.read(&mut [0]).unwrap(), 0);
        });

        let error = tokio::time::timeout(
            TIMEOUT,
            Connection::connect_with_runtime(
                &uri,
                properties(0).with_auth_provider(FailingAuth { fail_starter }),
                Runtime::tokio_current(),
            ),
        )
        .await
        .expect("authentication error left connect pending")
        .unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::AuthProviderError(_)));
        server.join().unwrap();
    }
}

#[tokio::test]
async fn initial_half_close_retries_handshake_on_new_socket() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let mut stream = accept(&listener);
            read_header(&mut stream);
            stream.shutdown(Shutdown::Write).unwrap();
            // Keep the read half open until the client abandons this socket.
            assert_eq!(stream.read(&mut [0]).unwrap(), 0);
        }
        let mut stream = accept(&listener);
        read_header(&mut stream);
        send_method(
            &mut stream,
            connection::AMQPMethod::Start(connection::Start {
                version_major: 0,
                version_minor: 9,
                mechanisms: "PLAIN".into(),
                locales: "en_US".into(),
                ..Default::default()
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::StartOk(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::Tune(connection::Tune {
                channel_max: 16,
                frame_max: 4096,
                heartbeat: 0,
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::TuneOk(_)
        ));
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Open(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::OpenOk(connection::OpenOk {}),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::Close(_)
        ));
        send_method(
            &mut stream,
            connection::AMQPMethod::CloseOk(connection::CloseOk {}),
        );
    });

    let connection = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, properties(2), Runtime::tokio_current()),
    )
    .await
    .expect("initial connection stayed pending")
    .expect("initial connection failed instead of retrying");
    assert!(connection.status().connected());
    tokio::time::timeout(TIMEOUT, connection.close(200, "test complete".into()))
        .await
        .unwrap()
        .unwrap();
    server.join().unwrap();
}

#[tokio::test]
async fn initial_half_close_rejects_when_retries_are_exhausted() {
    for retries in [0, 2] {
        let listener = listener();
        let uri = format!("amqp://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for _ in 0..=retries {
                let mut stream = accept(&listener);
                read_header(&mut stream);
                stream.shutdown(Shutdown::Write).unwrap();
                assert_eq!(stream.read(&mut [0]).unwrap(), 0);
            }
        });
        let error = tokio::time::timeout(
            TIMEOUT,
            Connection::connect_with_runtime(&uri, properties(retries), Runtime::tokio_current()),
        )
        .await
        .expect("exhausted connection stayed pending")
        .unwrap_err();
        assert!(matches!(error.kind(), ErrorKind::IOError(error)
            if error.kind() == std::io::ErrorKind::ConnectionAborted));
        server.join().unwrap();
    }
}

#[tokio::test]
async fn half_close_after_start_rejects_instead_of_replaying_authentication() {
    let listener = listener();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&listener);
        read_header(&mut stream);
        send_method(
            &mut stream,
            connection::AMQPMethod::Start(connection::Start {
                version_major: 0,
                version_minor: 9,
                mechanisms: "PLAIN".into(),
                locales: "en_US".into(),
                ..Default::default()
            }),
        );
        assert!(matches!(
            read_method(&mut stream),
            connection::AMQPMethod::StartOk(_)
        ));
        stream.shutdown(Shutdown::Write).unwrap();
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    });
    let error = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_with_runtime(&uri, properties(2), Runtime::tokio_current()),
    )
    .await
    .expect("handshake stayed pending after EOF")
    .unwrap_err();
    assert!(matches!(error.kind(), ErrorKind::IOError(error)
        if error.kind() == std::io::ErrorKind::ConnectionAborted));
    server.join().unwrap();
}
