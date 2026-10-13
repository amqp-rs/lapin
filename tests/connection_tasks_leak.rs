#![cfg(feature = "tokio")]

//! Regression test for https://github.com/amqp-rs/lapin/issues/474: each
//! connect/close cycle used to leave the tasks running the connection steps
//! (start-ok, open) parked forever.

use amq_protocol::{
    frame::{AMQPFrame, gen_frame, parse_frame},
    protocol::{AMQPClass, connection},
};
use lapin::{Connection, ConnectionProperties};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{Duration, Instant},
};

const CYCLES: usize = 20;
const TIMEOUT: Duration = Duration::from_secs(5);

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

fn serve_handshake_and_close(mut stream: TcpStream) {
    stream.set_read_timeout(Some(TIMEOUT)).unwrap();
    stream.set_write_timeout(Some(TIMEOUT)).unwrap();
    let mut header = [0; 8];
    stream.read_exact(&mut header).unwrap();
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
}

#[tokio::test(flavor = "multi_thread")]
async fn connect_close_cycles_do_not_leak_tasks() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let uri = format!("amqp://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for _ in 0..CYCLES {
            serve_handshake_and_close(listener.accept().unwrap().0);
        }
    });

    let metrics = tokio::runtime::Handle::current().metrics();
    let baseline = metrics.num_alive_tasks();
    for _ in 0..CYCLES {
        let conn = Connection::connect(&uri, ConnectionProperties::default())
            .await
            .unwrap();
        conn.close(200, "bye".into()).await.unwrap();
    }
    server.join().unwrap();

    let deadline = Instant::now() + TIMEOUT;
    while metrics.num_alive_tasks() > baseline && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(metrics.num_alive_tasks(), baseline);
}
