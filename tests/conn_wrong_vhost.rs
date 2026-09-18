use lapin::{Connection, ConnectionProperties, Result};

async fn tokio_main() -> Result<()> {
    let uri = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into())
        + "this-vhost-should-not-exist";
    let options = ConnectionProperties::default();
    let _connection = Connection::connect(&uri, options).await?;
    Ok(())
}

#[tokio::test]
async fn connection() {
    let res = tokio_main().await;
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.is_amqp_hard_error());
    if let lapin::ErrorKind::ProtocolError(e) = err.kind() {
        if let lapin::protocol::AMQPErrorKind::Hard(e) = e.kind() {
            assert_eq!(*e, lapin::protocol::AMQPHardError::NOTALLOWED);
        } else {
            unreachable!();
        }
    } else {
        unreachable!();
    }
}
