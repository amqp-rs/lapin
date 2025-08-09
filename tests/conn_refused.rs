use lapin::{Connection, ConnectionProperties, Result};

async fn tokio_main() -> Result<()> {
    let uri = "amqp://demo:demo@localhost:5673";
    let options = ConnectionProperties::default();
    let _connection = Connection::connect(uri, options).await?;
    Ok(())
}

#[test]
fn connection() {
    let res = tokio::runtime::Runtime::new()
        .expect("failed to build tokio runtime")
        .block_on(tokio_main());
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.is_io_error());
    if let lapin::ErrorKind::IOError(e) = err.kind() {
        assert_eq!(e.kind(), std::io::ErrorKind::ConnectionRefused);
    } else {
        unreachable!();
    }
}
