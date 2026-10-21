#![cfg(feature = "tokio")]

//! Cluster (multi-node) connection tests, using fake brokers so that no RabbitMQ
//! instance is required.

use amq_protocol::{
    frame::{AMQPFrame, gen_frame, parse_frame},
    protocol::{AMQPClass, connection},
};
use async_rs::Runtime;
use lapin::{
    Connection, ConnectionProperties, Error, ErrorKind,
    tcp::{AMQPUriTcpExt, OwnedTLSConfig},
    uri::AMQPUri,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(5);

fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(TIMEOUT)).unwrap();
                stream.set_write_timeout(Some(TIMEOUT)).unwrap();
                return stream;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "client never connected");
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
    let mut header = [0; 7];
    stream.read_exact(&mut header).unwrap();
    let size = u32::from_be_bytes(header[3..7].try_into().unwrap()) as usize;
    assert!(size < 4096, "unexpected frame size: {size}");
    let mut bytes = vec![0; 7 + size + 1];
    bytes[..7].copy_from_slice(&header);
    stream.read_exact(&mut bytes[7..]).unwrap();
    match parse_frame(bytes.as_slice()).unwrap().1 {
        AMQPFrame::Method(0, AMQPClass::Connection(method)) => method,
        frame => panic!("unexpected frame: {frame:?}"),
    }
}

/// Accept a client and perform the whole AMQP connection handshake with it.
fn accept_and_handshake(listener: &TcpListener) -> TcpStream {
    let mut stream = accept(listener);
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
    stream
}

/// Wait for the client to close the connection gracefully.
fn expect_close(stream: &mut TcpStream) {
    assert!(matches!(
        read_method(stream),
        connection::AMQPMethod::Close(_)
    ));
    send_method(
        stream,
        connection::AMQPMethod::CloseOk(connection::CloseOk {}),
    );
}

fn listener() -> TcpListener {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn uri(listener: &TcpListener) -> String {
    format!("amqp://{}", listener.local_addr().unwrap())
}

/// A URI on which nothing listens.
fn dead_uri() -> String {
    uri(&listener())
}

fn properties(retries: usize) -> ConnectionProperties {
    ConnectionProperties::default().configure_backoff(|backoff| {
        backoff
            .with_min_delay(Duration::from_millis(1))
            .with_max_delay(Duration::from_millis(1))
            .with_max_times(retries)
    })
}

#[tokio::test]
async fn fails_when_all_nodes_are_down() {
    let uris = [dead_uri(), dead_uri()];
    let error = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_cluster_with_runtime(&uris, properties(1), Runtime::tokio_current()),
    )
    .await
    .expect("cluster connection stayed pending")
    .unwrap_err();
    assert!(
        matches!(error.kind(), ErrorKind::IOError(_)),
        "unexpected error: {error:?}"
    );
}

#[tokio::test]
async fn rejects_inconsistent_cluster_uris() {
    for uris in [
        vec![],
        vec![
            "amqp://127.0.0.1:1/%2f".to_owned(),
            "amqp://127.0.0.1:2/other".to_owned(),
        ],
        vec![
            "amqp://user:pass@127.0.0.1:1".to_owned(),
            "amqp://other:pass@127.0.0.1:2".to_owned(),
        ],
    ] {
        let error = Connection::connect_cluster_with_runtime(
            &uris,
            properties(0),
            Runtime::tokio_current(),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error.kind(), ErrorKind::IOError(error) if error.kind() == std::io::ErrorKind::Other),
            "unexpected error: {error:?}"
        );
    }
}

#[tokio::test]
async fn connection_timeout_skips_unresponsive_node() {
    let live = listener();
    let live_port = live.local_addr().unwrap().port();
    let uris: Vec<AMQPUri> = [
        // Unresponsive node: our transport never completes for it.
        "amqp://127.0.0.1:1?connection_timeout=50".to_owned(),
        format!("{}?connection_timeout=5000", uri(&live)),
    ]
    .iter()
    .map(|uri| uri.parse().unwrap())
    .collect();
    let server = thread::spawn(move || {
        let mut stream = accept_and_handshake(&live);
        expect_close(&mut stream);
    });

    let connection = tokio::time::timeout(
        TIMEOUT,
        Connection::cluster_connector(
            uris,
            Runtime::tokio_current(),
            async move |uri, runtime| {
                if uri.authority.port != live_port {
                    std::future::pending::<()>().await;
                }
                AMQPUriTcpExt::connect_with_config_async(
                    &uri,
                    OwnedTLSConfig::default().as_ref(),
                    &runtime,
                )
                .await
                .map_err(Error::from)
            },
            properties(0),
        ),
    )
    .await
    .expect("connection_timeout was not honoured")
    .expect("cluster connection failed");
    assert!(connection.status().connected());
    tokio::time::timeout(TIMEOUT, connection.close(200, "test complete".into()))
        .await
        .unwrap()
        .unwrap();
    server.join().unwrap();
}
