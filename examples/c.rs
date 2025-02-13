use futures_lite::StreamExt;
use lapin::{Connection, ConnectionProperties, options::*, types::FieldTable};
use tracing::info;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") };
    }

    tracing_subscriber::fmt::init();

    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".into());
    let recovery_config = lapin::RecoveryConfig::default().auto_recover_channels();

    async_global_executor::block_on(async {
        let conn = Connection::connect(
            &addr,
            ConnectionProperties::default().with_experimental_recovery_config(recovery_config),
        )
        .await
        .expect("connection error");

        info!("CONNECTED");

        //receive channel
        let channel = conn.create_channel().await.expect("create_channel");
        info!(state=?conn.status().state());

        let queue = channel
            .queue_declare(
                "hello-recover",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("queue_declare");
        info!(state=?conn.status().state());
        info!(?queue, "Declared queue");

        let ch = channel.clone();
        async_global_executor::spawn(async move {
            loop {
                async_io::Timer::after(std::time::Duration::from_secs(1)).await;
                info!("Trigger failure");
                assert!(
                    ch.queue_declare(
                        "fake queue",
                        QueueDeclareOptions {
                            passive: true,
                            ..QueueDeclareOptions::default()
                        },
                        FieldTable::default(),
                    )
                    .await
                    .is_err()
                );
            }
        })
        .detach();

        info!("will consume");
        let mut consumer = channel
            .basic_consume(
                "hello-recover",
                "my_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .expect("basic_consume");
        info!(state=?conn.status().state());

        let mut count = 0;
        while let Some(delivery) = consumer.next().await {
            info!(message=?delivery, "received message");
            if let Ok(delivery) = delivery {
                let data = str::from_utf8(&delivery.data).expect("invalid utf8 data");
                println!("{}", data);
                delivery
                    .ack(BasicAckOptions::default())
                    .await
                    .expect("basic_ack");
                count += 1;
                if data == "STOP" {
                    println!("Received {} msgs", count);
                    break;
                }
            }
        }
    })
}
