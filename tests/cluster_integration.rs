#![cfg(all(feature = "tokio", feature = "testcontainers-tests"))]

//! Cluster (multi-node) connection tests against a real 3-node RabbitMQ cluster.
//!
//! Each test starts its own cluster with [testcontainers], so they need a Docker
//! compatible engine. With podman, expose its socket first:
//!
//! ```sh
//! systemctl --user enable --now podman.socket
//! export DOCKER_HOST=unix://$XDG_RUNTIME_DIR/podman/podman.sock
//! cargo test --test cluster_integration --features testcontainers-tests
//! ```

use futures_lite::stream::StreamExt;
use lapin::{
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties, Consumer,
    options::*,
    types::{AMQPValue, FieldTable},
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant, SystemTime},
};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    bollard::{self, query_parameters::RemoveContainerOptionsBuilder},
    core::{
        CmdWaitFor, ExecCommand, IntoContainerPort, WaitFor,
        error::{ClientError, TestcontainersError},
    },
    runners::AsyncRunner,
};

const IMAGE: &str = "rabbitmq";
const TAG: &str = "4";
const COOKIE: &str = "lapin-cluster-test";
const AMQP_PORT: u16 = 5672;
const NODES: usize = 3;
const TIMEOUT: Duration = Duration::from_secs(60);
const QUEUE: &str = "lapin-cluster-test";

struct Cluster {
    nodes: Vec<ContainerAsync<GenericImage>>,
    hostnames: Vec<String>,
    ports: Vec<u16>,
}

impl Cluster {
    async fn start() -> Self {
        static CLUSTERS: AtomicUsize = AtomicUsize::new(0);
        let id = format!(
            "lapin-{}-{}-{}",
            std::process::id(),
            SystemTime::UNIX_EPOCH.elapsed().unwrap().as_millis(),
            CLUSTERS.fetch_add(1, Ordering::Relaxed),
        );
        let hostnames = (1..=NODES)
            .map(|n| format!("{id}-rabbit{n}"))
            .collect::<Vec<_>>();
        // A killed node can vanish without its peers noticing the connection drop, they
        // then take `net_ticktime` (60s by default) to declare it down.
        let erl_args = format!("-setcookie {COOKIE} -kernel net_ticktime 4");
        let starting = hostnames
            .iter()
            .map(|hostname| {
                tokio::spawn(start_node(id.clone(), hostname.clone(), erl_args.clone()))
            })
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        let mut ports = Vec::new();
        for node in starting {
            let node = node.await.unwrap();
            ports.push(node.get_host_port_ipv4(AMQP_PORT).await.unwrap());
            nodes.push(node);
        }

        let cluster = Self {
            nodes,
            hostnames,
            ports,
        };
        let seed = format!("rabbit@{}", cluster.hostnames[0]);
        for n in 1..NODES {
            cluster.rabbitmqctl(n, &["stop_app"]).await;
            cluster.rabbitmqctl(n, &["join_cluster", &seed]).await;
            cluster.rabbitmqctl(n, &["start_app"]).await;
            cluster.rabbitmqctl(n, &["await_startup"]).await;
        }
        cluster
    }

    async fn rabbitmqctl(&self, node: usize, args: &[&str]) -> String {
        let cmd = ["rabbitmqctl", "--quiet"].iter().chain(args).copied();
        let mut result = self.nodes[node]
            .exec(ExecCommand::new(cmd).with_cmd_ready_condition(CmdWaitFor::exit()))
            .await
            .unwrap();
        let stdout = String::from_utf8(result.stdout_to_vec().await.unwrap()).unwrap();
        let stderr = String::from_utf8(result.stderr_to_vec().await.unwrap()).unwrap();
        assert_eq!(
            result.exit_code().await.unwrap(),
            Some(0),
            "rabbitmqctl {args:?} failed on node {node}: {stdout}{stderr}"
        );
        stdout
    }

    fn uris(&self, connection_timeout_ms: u64) -> Vec<String> {
        self.ports
            .iter()
            .map(|port| {
                format!("amqp://127.0.0.1:{port}/%2f?connection_timeout={connection_timeout_ms}")
            })
            .collect()
    }

    /// The node names the client connections are opened on, as seen by the cluster.
    async fn connection_nodes(&self, via: usize) -> Vec<String> {
        // Connection pids look like `<rabbit@host.1760000000.1234.0>`.
        self.rabbitmqctl(via, &["list_connections", "pid", "--no-table-headers"])
            .await
            .lines()
            .filter_map(|pid| pid.trim().strip_prefix('<')?.split('.').next())
            .map(str::to_owned)
            .collect()
    }

    /// Waits for the surviving nodes to notice that `node` went down.
    async fn wait_node_down(&self, node: usize, via: usize) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let status = self
                .rabbitmqctl(via, &["cluster_status", "--formatter", "json"])
                .await;
            let status: serde_json::Value = serde_json::from_str(&status).unwrap();
            let running = status["running_nodes"].as_array().unwrap();
            if !running.contains(&self.node_name(node).into()) {
                return;
            }
            assert!(Instant::now() < deadline, "node {node} never seen as down");
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    fn node_name(&self, node: usize) -> String {
        format!("rabbit@{}", self.hostnames[node])
    }

    async fn kill(&self, node: usize) {
        self.nodes[node].stop_with_timeout(Some(0)).await.unwrap();
    }

    async fn shutdown(&self, node: usize) {
        self.nodes[node].stop_with_timeout(Some(30)).await.unwrap();
    }
}

async fn start_node(
    network: String,
    hostname: String,
    erl_args: String,
) -> ContainerAsync<GenericImage> {
    let mut attempt = 0;
    loop {
        let request = GenericImage::new(IMAGE, TAG)
            .with_exposed_port(AMQP_PORT.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Server startup complete"))
            .with_network(&network)
            .with_container_name(&hostname)
            .with_hostname(&hostname)
            .with_env_var("RABBITMQ_SERVER_ADDITIONAL_ERL_ARGS", &erl_args)
            .with_env_var("RABBITMQ_CTL_ERL_ARGS", &erl_args)
            .with_startup_timeout(TIMEOUT);
        match request.start().await {
            Ok(node) => return node,
            // Rootless podman sometimes fails to set up the port forwarding when several
            // containers start at once ("rootlessport conflict"). The container is left
            // behind in the created state, so remove it before trying again.
            Err(error @ TestcontainersError::Client(ClientError::StartContainer(_)))
                if attempt < 3 =>
            {
                attempt += 1;
                eprintln!("failed to start {hostname} (attempt {attempt}): {error}");
                let docker = bollard::Docker::connect_with_defaults().unwrap();
                let options = RemoveContainerOptionsBuilder::new().force(true).build();
                let _ = docker.remove_container(&hostname, Some(options)).await;
            }
            Err(error) => panic!(
                "failed to start {hostname}: {error}\n\
                 (with podman, set DOCKER_HOST=unix://$XDG_RUNTIME_DIR/podman/podman.sock)"
            ),
        }
    }
}

fn properties() -> ConnectionProperties {
    ConnectionProperties::default()
        .enable_auto_recover()
        .configure_backoff(|backoff| {
            backoff
                .with_min_delay(Duration::from_millis(100))
                .with_max_delay(Duration::from_secs(1))
                .with_max_times(60)
        })
}

async fn connect(cluster: &Cluster) -> Connection {
    tokio::time::timeout(
        TIMEOUT,
        Connection::connect_cluster_with_runtime(
            &cluster.uris(2000),
            properties(),
            async_rs::Runtime::tokio_current(),
        ),
    )
    .await
    .expect("cluster connection stayed pending")
    .expect("cluster connection failed")
}

/// Declares a quorum queue (replicated across the cluster) with a consumer on it, and
/// returns a publishing channel (with publisher confirms) along with the consumer.
async fn setup(connection: &Connection) -> (Channel, Channel, Consumer) {
    let publisher = connection.create_channel().await.unwrap();
    publisher
        .confirm_select(ConfirmSelectOptions::default())
        .await
        .unwrap();
    let mut arguments = FieldTable::default();
    arguments.insert(
        "x-queue-type".into(),
        AMQPValue::LongString("quorum".into()),
    );
    publisher
        .queue_declare(QUEUE.into(), QueueDeclareOptions::durable(), arguments)
        .await
        .unwrap();

    let consumer_channel = connection.create_channel().await.unwrap();
    let consumer = consumer_channel
        .basic_consume(
            QUEUE.into(),
            "lapin-cluster-consumer".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .unwrap();
    (publisher, consumer_channel, consumer)
}

/// Publishes `payload` until the broker confirms it, recovering the channel as needed.
async fn publish(channel: &Channel, payload: &[u8]) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        assert!(Instant::now() < deadline, "could not publish {payload:?}");
        let result = async {
            channel
                .basic_publish(
                    "".into(),
                    QUEUE.into(),
                    BasicPublishOptions::default(),
                    payload,
                    BasicProperties::default(),
                )
                .await?
                .await
        };
        match tokio::time::timeout(TIMEOUT, result).await.unwrap() {
            Ok(Confirmation::Ack(_)) => return,
            Ok(confirmation) => panic!("unexpected confirmation: {confirmation:?}"),
            Err(error) => channel
                .wait_for_recovery(error)
                .await
                .expect("channel recovery failed"),
        }
    }
}

/// Waits for a delivery of `payload`, acking (and skipping) any redelivery of earlier
/// messages and recovering the channel as needed.
async fn receive(channel: &Channel, consumer: &mut Consumer, payload: &[u8]) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let delivery = tokio::time::timeout(remaining, consumer.next())
            .await
            .unwrap_or_else(|_| panic!("never received {payload:?}"))
            .expect("consumer was canceled");
        match delivery {
            Ok(delivery) => {
                delivery.ack(BasicAckOptions::default()).await.unwrap();
                if delivery.data == payload {
                    return;
                }
            }
            Err(error) => channel
                .wait_for_recovery(error)
                .await
                .expect("channel recovery failed"),
        }
    }
}

async fn wait_connected(connection: &Connection) {
    let deadline = Instant::now() + TIMEOUT;
    while !connection.status().connected() {
        assert!(Instant::now() < deadline, "connection never recovered");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

enum Failure {
    /// SIGKILL: the socket just goes away.
    Kill,
    /// Graceful shutdown: the node closes the connection with CONNECTION_FORCED.
    Shutdown,
}

async fn fails_over(failure: Failure) {
    let cluster = Cluster::start().await;
    let connection = connect(&cluster).await;
    let (publisher, consumer_channel, mut consumer) = setup(&connection).await;
    assert_eq!(
        cluster.connection_nodes(1).await,
        [cluster.node_name(0)],
        "should connect to the first node"
    );

    publish(&publisher, b"before").await;
    receive(&consumer_channel, &mut consumer, b"before").await;

    match failure {
        Failure::Kill => cluster.kill(0).await,
        Failure::Shutdown => cluster.shutdown(0).await,
    }
    // Until then, the quorum queue still considers the consumer we had on the lost node
    // alive and may hand it our next message, only to hold it for that (now gone)
    // consumer for `consumer_disconnected_timeout` (60s by default) once it notices.
    cluster.wait_node_down(0, 1).await;

    publish(&publisher, b"after").await;
    receive(&consumer_channel, &mut consumer, b"after").await;
    wait_connected(&connection).await;
    assert_eq!(
        cluster.connection_nodes(1).await,
        [cluster.node_name(1)],
        "should fail over to the next node"
    );

    connection.close(200, "test complete".into()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn fails_over_when_node_is_killed() {
    fails_over(Failure::Kill).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn fails_over_when_node_shuts_down() {
    fails_over(Failure::Shutdown).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn connects_to_next_node_when_first_is_down() {
    let cluster = Cluster::start().await;
    cluster.kill(0).await;

    let connection = connect(&cluster).await;
    let (publisher, consumer_channel, mut consumer) = setup(&connection).await;
    assert_eq!(cluster.connection_nodes(1).await, [cluster.node_name(1)]);

    publish(&publisher, b"hello").await;
    receive(&consumer_channel, &mut consumer, b"hello").await;

    connection.close(200, "test complete".into()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "connection_timeout only covers opening the socket, not the AMQP handshake"]
async fn connection_timeout_skips_unresponsive_node() {
    let cluster = Cluster::start().await;
    // A paused node accepts TCP connections (the port is forwarded by the container
    // engine) but never answers the AMQP handshake.
    cluster.nodes[0].pause().await.unwrap();

    let started = Instant::now();
    let connection = tokio::time::timeout(
        TIMEOUT,
        Connection::connect_cluster_with_runtime(
            &cluster.uris(1000),
            ConnectionProperties::default(),
            async_rs::Runtime::tokio_current(),
        ),
    )
    .await
    .expect("connection_timeout was not honoured")
    .expect("cluster connection failed");
    assert!(connection.status().connected());
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "connecting took {:?}",
        started.elapsed()
    );
    assert_eq!(cluster.connection_nodes(1).await, [cluster.node_name(1)]);

    connection.close(200, "test complete".into()).await.unwrap();
    cluster.nodes[0].unpause().await.unwrap();
}
