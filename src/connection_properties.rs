use crate::{
    auth::AuthProvider,
    types::{AMQPValue, FieldTable, LongString, ShortString},
};
use backon::ExponentialBuilder;
use std::fmt;
use std::sync::Arc;

/// Configuration for establishing a [`Connection`] to the broker.
///
/// Build one with [`Default::default`] and then chain the `with_*` / `enable_*`
/// builder methods to customise it.
///
/// # Example
///
/// ```rust,no_run
/// use lapin::{Connection, ConnectionProperties, Result};
/// # async fn example() -> Result<()> {
/// let props = ConnectionProperties::default()
///     .with_connection_name("my-app".into())
///     .enable_auto_recover();
/// let conn = Connection::connect("amqp://localhost", props).await?;
/// # Ok(()) }
/// ```
///
/// [`Connection`]: crate::Connection
#[derive(Clone)]
pub struct ConnectionProperties {
    pub(crate) locale: ShortString,
    pub(crate) client_properties: FieldTable,
    pub(crate) auth_provider: Option<Arc<dyn AuthProvider>>,
    pub(crate) backoff: ExponentialBuilder,
    pub(crate) auto_recover: bool,
    backoff_configured: bool,
}

impl Default for ConnectionProperties {
    fn default() -> Self {
        Self {
            locale: "en_US".into(),
            client_properties: FieldTable::default(),
            auth_provider: None,
            backoff: ExponentialBuilder::default().with_max_times(0 /* no retry by default */),
            auto_recover: false,
            backoff_configured: false,
        }
    }
}

impl ConnectionProperties {
    /// Override the AMQP locale sent to the server (default: `"en_US"`).
    #[must_use]
    pub fn with_locale(mut self, locale: ShortString) -> Self {
        self.locale = locale;
        self
    }

    /// Add an arbitrary client property sent to the server during the AMQP handshake.
    #[must_use]
    pub fn with_client_property(mut self, key: ShortString, value: LongString) -> Self {
        self.client_properties
            .insert(key, AMQPValue::LongString(value));
        self
    }

    /// Set the connection name shown in the RabbitMQ management UI.
    #[must_use]
    pub fn with_connection_name(self, connection_name: LongString) -> Self {
        self.with_client_property("connection_name".into(), connection_name)
    }

    /// Use a custom authentication provider instead of the default PLAIN/AMQPLAIN mechanism.
    ///
    /// See [`crate::auth::AuthProvider`] for how to implement one.
    #[must_use]
    pub fn with_auth_provider<AP: AuthProvider>(mut self, provider: AP) -> Self {
        self.auth_provider = Some(Arc::new(provider));
        self
    }

    /// Replace the TCP reconnect backoff strategy entirely.
    ///
    /// The default has zero retries; calling [`enable_auto_recover`] bumps it
    /// to 16. Use this when you need full control over the retry schedule.
    ///
    /// [`enable_auto_recover`]: Self::enable_auto_recover
    #[must_use]
    pub fn with_backoff(mut self, backoff: ExponentialBuilder) -> Self {
        self.backoff = backoff;
        self.backoff_configured = true;
        self
    }

    /// Modify the TCP reconnect backoff strategy via a closure.
    ///
    /// Receives the current [`ExponentialBuilder`] and must return a new one.
    #[must_use]
    pub fn configure_backoff(
        mut self,
        conf: impl Fn(ExponentialBuilder) -> ExponentialBuilder,
    ) -> Self {
        self.backoff = conf(self.backoff);
        self.backoff_configured = true;
        self
    }

    /// Enable automatic connection and topology recovery after failures.
    ///
    /// When enabled the IO loop will transparently reconnect on network errors
    /// and replay the exchange/queue/binding/consumer topology, along with the
    /// publisher confirms and QoS settings, on the new connection. Also sets a
    /// default TCP reconnect backoff (16 attempts with exponential delay)
    /// unless you have already called [`with_backoff`] or
    /// [`configure_backoff`].
    ///
    /// After catching a recoverable error on a channel, call
    /// [`Channel::wait_for_recovery`] to wait until recovery is complete.
    ///
    /// [`with_backoff`]: Self::with_backoff
    /// [`configure_backoff`]: Self::configure_backoff
    /// [`Channel::wait_for_recovery`]: crate::Channel::wait_for_recovery
    #[must_use]
    pub fn enable_auto_recover(mut self) -> Self {
        self.auto_recover = true;
        if !self.backoff_configured {
            // Arbitrary value to make sure we retry the TCP connection itself a few times
            // Don't touch anything if the user already explicitely configured the backoff
            self.backoff = self.backoff.with_max_times(16);
            self.backoff_configured = true;
        }
        self
    }
}

impl fmt::Debug for ConnectionProperties {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionProperties")
            .field("locale", &self.locale)
            .field("client_properties", &self.client_properties)
            .field("backoff", &self.backoff)
            .field("auto_recover", &self.auto_recover)
            .finish()
    }
}
