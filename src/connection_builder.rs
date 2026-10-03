use crate::{
    Connection, ConnectionProperties, Error, Result, connection::Connect, runtime,
    tcp::OwnedTLSConfig, uri::AMQPUri,
};

use async_rs::{Runtime, traits::*};
use std::fmt;

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
pub struct ConnectionBuilder<RK: RuntimeKit + Send + Sync + Clone + 'static> {
    runtime: Runtime<RK>,
    uri: UriBuilder,
    properties: Option<ConnectionProperties>,
    tls_config: Option<OwnedTLSConfig>,
}

impl<RK: RuntimeKit + Send + Sync + Clone + 'static> fmt::Debug for ConnectionBuilder<RK> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionBuilder")
            .field("uri_configured", &!matches!(self.uri, UriBuilder::Unset))
            .field("properties_configured", &self.properties.is_some())
            .field("tls_configured", &self.tls_config.is_some())
            .finish()
    }
}

/// A [`ConnectionBuilder`] using the crate's default async runtime.
pub type DefaultConnectionBuilder = ConnectionBuilder<runtime::DefaultRuntimeKit>;

#[derive(Clone, Default)]
enum UriBuilder {
    Str(String),
    Uri(AMQPUri),
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
            UriBuilder::Unset => Err(Error::other("No AMQPUri given to ConnectionBuilder")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DefaultConnectionBuilder;

    #[test]
    fn debug_redacts_uri_credentials() {
        let builder = DefaultConnectionBuilder::new()
            .unwrap()
            .with_uri_str("amqp://user:secret@localhost".into());
        let output = format!("{builder:?}");
        assert!(!output.contains("secret"));
        assert!(!output.contains("user"));
        assert!(output.contains("uri_configured: true"));
    }
}
