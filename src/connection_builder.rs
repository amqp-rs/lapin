use crate::{
    Connection, ConnectionProperties, Error, Result, connection::Connect, runtime,
    tcp::OwnedTLSConfig, uri::AMQPUri,
};

use async_rs::{Runtime, traits::*};

/// Builder for [`Connection`] that collects URI, properties, and TLS config
/// before connecting.
///
/// Use [`DefaultConnectionBuilder::new`] for the common case (default runtime).
/// Use [`ConnectionBuilder::new_with_runtime`] to supply a custom runtime.
///
/// # Example
///
/// ```rust,no_run
/// use lapin::{DefaultConnectionBuilder, Result};
/// # async fn example() -> Result<()> {
/// let conn = DefaultConnectionBuilder::new()?
///     .with_uri_str("amqp://localhost".into())
///     .connect()
///     .await?;
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct ConnectionBuilder<RK: RuntimeKit + Send + Sync + Clone + 'static> {
    runtime: Runtime<RK>,
    uri: UriBuilder,
    properties: Option<ConnectionProperties>,
    tls_config: Option<OwnedTLSConfig>,
}

/// A [`ConnectionBuilder`] using the crate's default async runtime.
pub type DefaultConnectionBuilder = ConnectionBuilder<runtime::DefaultRuntimeKit>;

#[derive(Debug, Clone, Default)]
enum UriBuilder {
    Str(String),
    Uri(AMQPUri),
    ClusterStr(Vec<String>),
    ClusterUri(Vec<AMQPUri>),
    #[default]
    Unset,
}

impl DefaultConnectionBuilder {
    /// Create a builder using the crate's default async runtime.
    pub fn new() -> Result<Self> {
        Ok(Self::new_with_runtime(runtime::default_runtime()?))
    }
}

impl<RK: RuntimeKit + Send + Sync + Clone + 'static> ConnectionBuilder<RK> {
    /// Create a builder with a custom runtime.
    pub fn new_with_runtime(runtime: Runtime<RK>) -> Self {
        ConnectionBuilder {
            runtime,
            uri: UriBuilder::default(),
            properties: None,
            tls_config: None,
        }
    }

    /// Set the broker URI from a pre-parsed [`AMQPUri`].
    pub fn with_uri(mut self, uri: AMQPUri) -> Self {
        self.uri = UriBuilder::Uri(uri);
        self
    }

    /// Set the broker URI from a string (parsed lazily at [`connect`] time).
    ///
    /// [`connect`]: Self::connect
    pub fn with_uri_str(mut self, uri: String) -> Self {
        self.uri = UriBuilder::Str(uri);
        self
    }

    /// Set the URIs of the nodes of a cluster from pre-parsed [`AMQPUri`]s.
    ///
    /// See [`Connection::connect_cluster`] for the cluster semantics.
    pub fn with_cluster_uris(mut self, uris: Vec<AMQPUri>) -> Self {
        self.uri = UriBuilder::ClusterUri(uris);
        self
    }

    /// Set the URIs of the nodes of a cluster from strings (parsed lazily at
    /// [`connect`] time).
    ///
    /// See [`Connection::connect_cluster`] for the cluster semantics.
    ///
    /// [`connect`]: Self::connect
    pub fn with_cluster_uri_strs(mut self, uris: Vec<String>) -> Self {
        self.uri = UriBuilder::ClusterStr(uris);
        self
    }

    /// Override the default [`ConnectionProperties`].
    pub fn with_properties(mut self, properties: ConnectionProperties) -> Self {
        self.properties = Some(properties);
        self
    }

    /// Override the default TLS configuration.
    pub fn with_tls_config(mut self, tls_config: OwnedTLSConfig) -> Self {
        self.tls_config = Some(tls_config);
        self
    }

    /// Establish the connection with the configured parameters.
    pub async fn connect(&self) -> Result<Connection> {
        let properties = self.properties.clone().unwrap_or_default();
        let tls_config = self.tls_config.clone().unwrap_or_default();
        let runtime = self.runtime.clone();

        match self.uri.clone() {
            UriBuilder::Str(uri) => {
                uri.connect_with_config(properties, tls_config, runtime)
                    .await
            }
            UriBuilder::Uri(uri) => {
                uri.connect_with_config(properties, tls_config, runtime)
                    .await
            }
            UriBuilder::ClusterStr(uris) => {
                Connection::connect_cluster_with_config(&uris, properties, tls_config, runtime)
                    .await
            }
            UriBuilder::ClusterUri(uris) => {
                uris.connect_with_config(properties, tls_config, runtime)
                    .await
            }
            UriBuilder::Unset => Err(Error::other("No AMQPUri given to ConnectionBuilder")),
        }
    }
}
