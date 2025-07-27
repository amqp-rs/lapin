use lapin::{
    BasicProperties, Connection, ConnectionProperties, message::DeliveryResult, options::*,
    publisher_confirm::Confirmation, types::FieldTable,
};
use tracing::info;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
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

        {
            let channel1 = conn.create_channel().await.expect("create_channel");
            let channel2 = conn.create_channel().await.expect("create_channel");
            channel1
                .confirm_select(ConfirmSelectOptions::default())
                .await
                .expect("confirm_select");
            channel1
                .queue_declare(
                    "recover-test",
                    QueueDeclareOptions::default(),
                    FieldTable::default(),
                )
                .await
                .expect("queue_declare");

            info!("will consume");
            let channel = channel2.clone();
            channel2
                .basic_consume(
                    "recover-test",
                    "my_consumer",
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await
                .expect("basic_consume")
                .set_delegate(move |delivery: DeliveryResult| {
                    let channel = channel.clone();
                    async move {
                        info!(message=?delivery, "received message");
                        if let Ok(Some(delivery)) = delivery {
                            delivery
                                .ack(BasicAckOptions::default())
                                .await
                                .expect("basic_ack");
                            if &delivery.data[..] == b"after" {
                                channel
                                    .basic_cancel("my_consumer", BasicCancelOptions::default())
                                    .await
                                    .expect("basic_cancel");
                            }
                        }
                    }
                });

            info!("will publish");
            let confirm = channel1
                .basic_publish(
                    "",
                    "recover-test",
                    BasicPublishOptions::default(),
                    b"before",
                    BasicProperties::default(),
                )
                .await
                .expect("basic_publish")
                .await
                .expect("publisher-confirms");
            assert_eq!(confirm, Confirmation::Ack(None));

            info!("before fail");
            assert!(
                channel1
                    .queue_declare(
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
            info!("after fail");

            info!("publish after");
            let confirm = channel1
                .basic_publish(
                    "",
                    "recover-test",
                    BasicPublishOptions::default(),
                    b"after",
                    BasicProperties::default(),
                )
                .await
                .expect("basic_publish")
                .await
                .expect("publisher-confirms");
            assert_eq!(confirm, Confirmation::Ack(None));
        }

        conn.run().expect("conn.run");
    });
}
