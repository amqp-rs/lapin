#![cfg(feature = "tokio")]

use amq_protocol::{
    frame::{AMQPFrame, gen_frame, parse_frame},
    protocol::{AMQPClass, connection},
};
use async_rs::Runtime;
use lapin::{Connection, ConnectionProperties, ErrorKind};
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
