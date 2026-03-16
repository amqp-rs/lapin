use async_rs::Runtime;
use lapin::{
    BasicProperties, Confirmation, Connection, ConnectionProperties, options::*, types::FieldTable,
};

/// Connections created via `connect_with_runtime` must outlive the runtime they
/// were called from: `InternalRPC::run` must be bound to the supplied runtime,
/// not the caller's ambient runtime.
#[test]
fn connection_survives_caller_runtime_shutdown() {
    let _ = tracing_subscriber::fmt::try_init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());

    let static_rt = tokio::runtime::Runtime::new().unwrap();
    let lapin_runtime = Runtime::tokio_with_handle(static_rt.handle().clone());

    let caller_rt = tokio::runtime::Runtime::new().unwrap();
    let connection = caller_rt.block_on(async {
        Connection::connect_with_runtime(&addr, ConnectionProperties::default(), lapin_runtime)
            .await
            .expect("connection error")
    });

    // Drop the caller runtime — kills any tasks mistakenly spawned on it.
    drop(caller_rt);

    static_rt.block_on(async {
        let channel = connection
            .create_channel()
            .await
            .expect("create_channel after caller runtime shutdown");

        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .expect("confirm_select");

        channel
            .queue_declare(
                "runtime-isolation-test".into(),
                QueueDeclareOptions {
                    auto_delete: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .expect("queue_declare");

        let confirm = channel
            .basic_publish(
                "".into(),
                "runtime-isolation-test".into(),
                BasicPublishOptions::default(),
                b"hello",
                BasicProperties::default(),
            )
            .await
            .expect("basic_publish")
            .await
            .expect("publisher confirm");

        assert_eq!(confirm, Confirmation::Ack(None));
    });
}
