use crate::types::LongString;
use amq_protocol::auth::{Credentials, SASLMechanism};

/// A trait used to integrate custom authentication process during connection
pub trait AuthProvider: Send + 'static {
    /// The String representation of the auth mechanism as understood by the RabbitMQ server
    fn mechanism(&self) -> String {
        SASLMechanism::External.name().to_string()
    }

    /// The initial data to provide to the RabbitMQ server for authentication
    fn auth_starter(&mut self) -> Result<String, String> {
        Ok(String::new())
    }

    /// The answer to the received challenge to forward to the RabbitMQ server (Connection.SecureOk)
    fn continue_auth(&mut self, challenge: LongString) -> Result<String, String> {
        Err(format!(
            "Received Connection.Secure with challenge '{challenge}' but we don't know how to handle it for {0}.",
            self.mechanism(),
        ))
    }

    /// Does the current session need refreshing a token?
    fn needs_refresh(&self) -> bool {
        false
    }

    /// Refresh the current session token (Connection.UpdateSecret)
    fn refresh(&mut self) -> Result<String, String> {
        Err("Refresh not supported on current auth provider".to_string())
    }
}

/// Provider to create tokens used for (OAuth2) auth
pub trait TokenProvider: Send + 'static {
    /// Check whether the current token is expired
    fn expired(&self) -> bool;

    /// Create a new token
    fn create_token(&mut self) -> Result<String, String>;
}

pub(crate) struct DefaultAuthProvider {
    credentials: Credentials,
    mechanism: SASLMechanism,
}

impl DefaultAuthProvider {
    pub(crate) fn new(credentials: Credentials, mechanism: SASLMechanism) -> Self {
        Self {
            credentials,
            mechanism,
        }
    }
}

impl AuthProvider for DefaultAuthProvider {
    fn mechanism(&self) -> String {
        self.mechanism.name().to_string()
    }

    fn auth_starter(&mut self) -> Result<String, String> {
        Ok(self.credentials.sasl_auth_string(self.mechanism))
    }

    fn continue_auth(&mut self, challenge: LongString) -> Result<String, String> {
        if self.mechanism != SASLMechanism::RabbitCrDemo {
            return Err(format!(
                "Received invalid Connection.Secure with challenge '{challenge}' for SASL mechanism {0} with default provider.",
                self.mechanism
            ));
        }

        if String::from_utf8_lossy(challenge.as_bytes())
            != self.credentials.rabbit_cr_demo_challenge()
        {
            return Err(format!(
                "{0}: received invalid challenge '{challenge}'",
                self.mechanism,
            ));
        }

        Ok(self.credentials.rabbit_cr_demo_answer())
    }
}

/// OAuth2 token auth handler with expiry and refresh support
pub struct TokenAuthProvider<TP: TokenProvider> {
    token_provider: TP,
    mechanism: SASLMechanism,
}

impl<TP: TokenProvider> TokenAuthProvider<TP> {
    /// Create a new OAuth2 token auth provider
    pub fn new(token_provider: TP) -> Self {
        Self {
            token_provider,
            mechanism: SASLMechanism::default(),
        }
    }
}

impl<TP: TokenProvider> From<TP> for TokenAuthProvider<TP> {
    fn from(token_provider: TP) -> Self {
        Self::new(token_provider)
    }
}

impl<TP: TokenProvider> AuthProvider for TokenAuthProvider<TP> {
    fn mechanism(&self) -> String {
        self.mechanism.name().to_string()
    }

    fn auth_starter(&mut self) -> Result<String, String> {
        Ok(
            Credentials::new(String::new(), self.token_provider.create_token()?)
                .sasl_auth_string(self.mechanism),
        )
    }

    fn needs_refresh(&self) -> bool {
        self.token_provider.expired()
    }

    fn refresh(&mut self) -> Result<String, String> {
        self.token_provider.create_token()
    }
}

/// Default implementation for OAUth2 token refresh mechanism based on user-supplied callbacks
///
/// # Example
///
/// ```rust
/// use lapin::auth::{DefaultTokenProvider, TokenAuthProvider};
///
/// let auth_provider: TokenAuthProvider<_> = DefaultTokenProvider::new(
///     |_token| false /* never expire */,
///     || Ok("my new valid token".to_string()),
/// ).into();
/// ```
pub struct DefaultTokenProvider {
    expired: Box<dyn Fn(&str) -> bool + Send + 'static>,
    create_token: Box<dyn Fn() -> Result<String, String> + Send + 'static>,
    token: String,
}

impl DefaultTokenProvider {
    pub fn new<
        E: Fn(&str) -> bool + Send + 'static,
        CT: Fn() -> Result<String, String> + Send + 'static,
    >(
        expired: E,
        create_token: CT,
    ) -> Self {
        Self {
            expired: Box::new(expired),
            create_token: Box::new(create_token),
            token: String::new(),
        }
    }
}

impl TokenProvider for DefaultTokenProvider {
    fn expired(&self) -> bool {
        (self.expired)(&self.token)
    }

    fn create_token(&mut self) -> Result<String, String> {
        self.token = (self.create_token)()?;
        Ok(self.token.clone())
    }
}
