use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ExchangeKind {
    Custom(String),
    Direct,
    Fanout,
    Headers,
    Topic,
}

impl Default for ExchangeKind {
    fn default() -> Self {
        Self::Direct
    }
}

impl ExchangeKind {
    pub(crate) fn kind(&self) -> &str {
        match self {
            Self::Custom(c) => c.as_str(),
            Self::Direct => "direct",
            Self::Fanout => "fanout",
            Self::Headers => "headers",
            Self::Topic => "topic",
        }
    }
}
