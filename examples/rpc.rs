use futures_lite::stream::StreamExt;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
};
use tokio::time::{sleep, Duration};
use tracing::{error, info};
use uuid::Uuid;

async fn open_channel() -> Result<Channel, Box<dyn std::error::Error>> {
    let addr = std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672".into());
    let conn = Connection::connect(&addr, ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;
    channel
        .confirm_select(ConfirmSelectOptions::default())
        .await?;
    Ok(channel)
}

struct RpcClient {
    channel: Channel,
    reply_to: String,
}

impl RpcClient {
    async fn try_new() -> Result<Self, Box<dyn std::error::Error>> {
        let channel = open_channel().await?;

        channel
            .queue_declare(
                "rpc_queue",
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| format!("Failed to declare queue rpc_queue:{e}"))?;

        let reply_to = "rpc_queue_reply_to_".to_owned() + Uuid::new_v4().to_string().as_str();
        channel
            .queue_declare(
                reply_to.as_str(),
                QueueDeclareOptions {
                    exclusive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| format!("Failed to declare queue {reply_to}: {e}"))?;

        Ok(Self { channel, reply_to })
    }

    async fn rpc_call(&mut self, message: i64) -> Result<(), Box<dyn std::error::Error>> {
        let correlation_id = Uuid::new_v4().to_string();
        while {
            let properties = BasicProperties::default()
                .with_correlation_id(correlation_id.as_str().into())
                .with_reply_to(self.reply_to.as_str().into());
            info!(
                "RPC client: correlation_id {:?}",
                properties.correlation_id()
            );
            !self
                .channel
                .basic_publish(
                    "",
                    "rpc_queue",
                    BasicPublishOptions::default(),
                    message.to_string().as_bytes(),
                    properties,
                )
                .await
                .map_err(|e| format!("Failed to publish message: {e}"))?
                .await
                .map_err(|e| format!("Publish confirm error: {e}"))?
                .is_ack()
        } {
            error!("RPC client: did not get ack message, will retry sending it");
            sleep(Duration::from_millis(100)).await;
        }
        info!("RPC client: sent message: {}", message);

        let mut consumer = self
            .channel
            .basic_consume(
                self.reply_to.as_str(),
                "",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                format!(
                    "Failed to create consumer for queue {}: {}",
                    self.reply_to, e
                )
            })?;
        while let Some(delivery) = consumer.next().await {
            let delivery = delivery?;
            match delivery.properties.correlation_id() {
                Some(x) if x.as_str() == correlation_id => {}
                _ => {
                    error!("RPC client: invalid correlation_id in delivery");
                    delivery.ack(BasicAckOptions::default()).await?;
                    continue;
                }
            }

            info!(
                "RPC client: received message {}",
                std::str::from_utf8(&delivery.data[..])?
            );

            delivery.ack(BasicAckOptions::default()).await?;
            break;
        }

        Ok(())
    }
}

async fn server() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let channel = open_channel().await?;
        channel
            .queue_declare(
                "rpc_queue",
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| format!("Failed to declare queue rpc_queue:{e}"))?;

        let mut consumer = channel
            .basic_consume(
                "rpc_queue",
                "",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| format!("Failed to create consumer for queue \"rpc_queue\": {e}"))?;

        while let Some(delivery) = consumer.next().await {
            let delivery = delivery?;
            let received_message = std::str::from_utf8(&delivery.data[..])?;
            let send_message = received_message.parse::<i64>()?;
            let send_message = send_message + 100;
            let send_message = send_message.to_string();

            info!("RPC server: received message: {}", received_message);
            let correlation_id = match delivery.properties.correlation_id() {
                Some(correlation_id) => correlation_id,
                _ => {
                    error!("RPC server: invalid correlation_id in delivery");
                    delivery.ack(BasicAckOptions::default()).await?;
                    continue;
                }
            };
            let reply_to = match delivery.properties.reply_to() {
                Some(reply_to) => reply_to,
                _ => {
                    error!("RPC server: invalid correlation_id in delivery");
                    delivery.ack(BasicAckOptions::default()).await?;
                    continue;
                }
            };

            while {
                let properties =
                    BasicProperties::default().with_correlation_id(correlation_id.clone());
                info!(
                    "RPC server: correlation_id {:?}",
                    properties.correlation_id()
                );
                !channel
                    .basic_publish(
                        "",
                        reply_to.as_str(),
                        BasicPublishOptions::default(),
                        send_message.as_bytes(),
                        properties,
                    )
                    .await
                    .map_err(|e| format!("Failed to publish message: {e}"))?
                    .await
                    .map_err(|e| format!("Publish confirm error: {e}"))?
                    .is_ack()
            } {
                error!("RPC server: did not get ack message, will retry sending it");
                sleep(Duration::from_millis(100)).await;
            }
            info!("RPC server: sent message {}", send_message);

            delivery.ack(BasicAckOptions::default()).await?;
            break;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    // Delete the queue
    // before starting the server
    // open_channel()
    //     .await
    //     .map_err(|e| format!("Failed to open channel: {e}"))?
    //     .queue_delete("rpc_queue", QueueDeleteOptions::default())
    //     .await
    //     .map_err(|e| format!("Failed to delete queue rpc_queue: {e}"))?;

    // In practice, the server and client could run
    // on distinct machines
    tokio::join!(
        async {
            loop {
                if let Err(e) = server().await {
                    println!("Error in server: {e}.");
                }
                println!("Restarting server in 1 second.");
                sleep(Duration::from_secs(1)).await;
            }
        },
        async {
            // Create a client instance
            let mut client = loop {
                let client = RpcClient::try_new().await;
                match client {
                    Ok(client) => break client,
                    Err(e) => {
                        error!("Error creating client: {e}.");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            };

            // Send rpc requests and wait for the response
            for i in 0.. {
                match client.rpc_call(i).await {
                    Ok(_) => {
                        info!("Request successful");
                    }
                    Err(e) => {
                        error!("Client error while sending request: {e}.");
                    }
                }
                info!("Sending new request in 10 second.");
                sleep(Duration::from_secs(10)).await;
            }
        },
    );

    Ok(())
}
