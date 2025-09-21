use crate::{
    types::{LongString, ShortString},
    uri::AMQPUri,
};
use amq_protocol::auth::{Credentials, SASLMechanism};
use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};

/// A trait used to integrate custom authentication process during connection
pub trait AuthProvider: Send + Sync + 'static {
    /// The String representation of the auth mechanism as understood by the RabbitMQ server
    fn mechanism(&self) -> ShortString {
        SASLMechanism::External.name().into()
    }

    /// The initial data to provide to the RabbitMQ server for authentication
    fn auth_starter(&self) -> Result<LongString, String> {
        Ok("".into())
    }

    /// The answer to the received challenge to forward to the RabbitMQ server (Connection.SecureOk)
    fn continue_auth(&self, challenge: LongString) -> Result<LongString, String> {
        Err(format!(
            "Received Connection.Secure with challenge '{challenge}' but we don't know how to handle it for {0}.",
            self.mechanism(),
        ))
    }

    /// How long is the current session/token valid for? None means no expiration.
    fn valid_for(&self) -> Option<Duration> {
        None
    }

    /// Refresh the current session token (Connection.UpdateSecret)
    fn refresh(&self) -> Result<LongString, String> {
        Err("Refresh not supported on current auth provider".to_string())
    }
}

/// Provider to create tokens used for (OAuth2) auth
pub trait TokenProvider: Send + Sync + 'static {
    /// Check for how long the current token is still valid. `None` means no expiration.
    fn valid_for(&self) -> Option<Duration> {
        None
    }

    /// Create a new token
    fn create_token(&self) -> Result<LongString, String>;
}

pub(crate) struct DefaultAuthProvider {
    credentials: Credentials,
    mechanism: SASLMechanism,
}

impl DefaultAuthProvider {
    pub(crate) fn new(uri: &AMQPUri) -> Self {
        Self {
            credentials: uri.authority.userinfo.clone().into(),
            mechanism: uri.query.auth_mechanism.unwrap_or_default(),
        }
    }
}

impl AuthProvider for DefaultAuthProvider {
    fn mechanism(&self) -> ShortString {
        self.mechanism.name().into()
    }

    fn auth_starter(&self) -> Result<LongString, String> {
        Ok(self.credentials.sasl_auth_string(self.mechanism))
    }

    fn continue_auth(&self, challenge: LongString) -> Result<LongString, String> {
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
    fn mechanism(&self) -> ShortString {
        self.mechanism.name().into()
    }

    fn auth_starter(&self) -> Result<LongString, String> {
        Ok(
            Credentials::new(LongString::default(), self.token_provider.create_token()?)
                .sasl_auth_string(self.mechanism),
        )
    }

    fn valid_for(&self) -> Option<Duration> {
        self.token_provider.valid_for()
    }

    fn refresh(&self) -> Result<LongString, String> {
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
///     |_token| None /* never expire */,
///     || Ok("my new valid token".into()),
/// ).into();
/// ```
pub struct DefaultTokenProvider {
    #[allow(clippy::type_complexity)]
    valid_for: Box<dyn Fn(&LongString) -> Option<Duration> + Send + Sync + 'static>,
    create_token: Box<dyn Fn() -> Result<LongString, String> + Send + Sync + 'static>,
    token: Mutex<LongString>,
}

impl DefaultTokenProvider {
    pub fn new<
        VF: Fn(&LongString) -> Option<Duration> + Send + Sync + 'static,
        CT: Fn() -> Result<LongString, String> + Send + Sync + 'static,
    >(
        valid_for: VF,
        create_token: CT,
    ) -> Self {
        Self {
            valid_for: Box::new(valid_for),
            create_token: Box::new(create_token),
            token: Mutex::new(LongString::default()),
        }
    }

    fn lock_token(&self) -> MutexGuard<'_, LongString> {
        self.token.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl TokenProvider for DefaultTokenProvider {
    fn valid_for(&self) -> Option<Duration> {
        (self.valid_for)(&self.lock_token())
    }

    fn create_token(&self) -> Result<LongString, String> {
        let token = (self.create_token)()?;
        *self.lock_token() = token.clone();
        Ok(token)
    }
}
