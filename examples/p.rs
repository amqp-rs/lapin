use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use tracing::info;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let recovery_config = lapin::experimental::RecoveryConfig::default().auto_recover_channels();

    async_global_executor::block_on(async {
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default().with_experimental_recovery_config(recovery_config),
        )
        .await
        .expect("connection error");

        info!("CONNECTED");

        let channel1 = conn.create_channel().await.expect("create_channel");
        channel1
            .confirm_select(ConfirmSelectOptions::default())
            .await
            .expect("confirm_select");
        channel1
            .queue_declare(
                "hello-recover",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("queue_declare");

        let ch = channel1.clone();
        async_global_executor::spawn(async move {
            loop {
                async_io::Timer::after(std::time::Duration::from_secs(1)).await;
                info!("Trigger failure");
                assert!(ch
                    .queue_declare(
                        "fake queue",
                        QueueDeclareOptions {
                            passive: true,
                            ..QueueDeclareOptions::default()
                        },
                        FieldTable::default(),
                    )
                    .await
                    .is_err());
            }
        })
        .detach();

        let mut published = 0;
        let mut errors = 0;
        info!("will publish");
        loop {
            let res = channel1
                .basic_publish(
                    "",
                    "recover-test",
                    BasicPublishOptions::default(),
                    b"before",
                    BasicProperties::default(),
                )
                .await;
            let res = if let Ok(res) = res {
                res.await.map(|_| ())
            } else {
                res.map(|_| ())
            };
            match res {
                Ok(()) => {
                    println!("GOT OK");
                    published += 1;
                }
                Err(err) => {
                    println!("GOT ERROR");
                    if !err.is_amqp_soft_error() {
                        panic!("{}", err);
                    }
                    errors += 1;
                    if let Some(notifier) = err.notifier() {
                        notifier.await
                    }
                }
            }
            println!("Published {} with {} errors", published, errors);
        }
    });
}
