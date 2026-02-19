use crate::{
    Connection, ConnectionProperties, Result, connection::Connect, runtime, tcp::OwnedTLSConfig,
    uri::AMQPUri,
};

use async_rs::{Runtime, traits::*};
use std::io;

pub struct ConnectionBuilder<RK: RuntimeKit + Send + Sync + Clone + 'static> {
    runtime: Runtime<RK>,
    uri: UriBuilder,
    properties: Option<ConnectionProperties>,
    tls_config: Option<OwnedTLSConfig>,
}
pub type DefaultConnectionBuilder = ConnectionBuilder<runtime::DefaultRuntimeKit>;

#[derive(Clone, Default)]
enum UriBuilder {
    Str(String),
    Uri(AMQPUri),
    #[default]
    Unset,
}

impl DefaultConnectionBuilder {
    pub fn new() -> Result<Self> {
        Ok(Self::new_with_runtime(runtime::default_runtime()?))
    }
}

impl<RK: RuntimeKit + Send + Sync + Clone + 'static> ConnectionBuilder<RK> {
    pub fn new_with_runtime(runtime: Runtime<RK>) -> Self {
        ConnectionBuilder {
            runtime,
            uri: UriBuilder::default(),
            properties: None,
            tls_config: None,
        }
    }

    pub fn with_uri(mut self, uri: AMQPUri) -> Self {
        self.uri = UriBuilder::Uri(uri);
        self
    }

    pub fn with_uri_str(mut self, uri: String) -> Self {
        self.uri = UriBuilder::Str(uri);
        self
    }

    pub fn with_properties(mut self, properties: ConnectionProperties) -> Self {
        self.properties = Some(properties);
        self
    }

    pub fn with_tls_config(mut self, tls_config: OwnedTLSConfig) -> Self {
        self.tls_config = Some(tls_config);
        self
    }

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
            UriBuilder::Unset => {
                Err(io::Error::other("No AMQPUri given to ConnectionBuilder").into())
            }
        }
    }
}
