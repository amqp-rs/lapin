#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ExchangeKind {
    Custom(String),
    #[default]
    Direct,
    Fanout,
    Headers,
    Topic,
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
