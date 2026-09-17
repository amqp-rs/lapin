use crate::{Connection, PromiseResolver, auth::AuthProvider};
use std::{fmt, sync::Arc};

pub(crate) enum ConnectionStep {
    ProtocolHeader(PromiseResolver<Connection>, Connection),
    StartOk(
        PromiseResolver<Connection>,
        Connection,
        Arc<dyn AuthProvider>,
    ),
    SecureOk(
        PromiseResolver<Connection>,
        Connection,
        Arc<dyn AuthProvider>,
    ),
    Open(PromiseResolver<Connection>, Connection),
}

impl ConnectionStep {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            ConnectionStep::ProtocolHeader(..) => "ProtocolHeader",
            ConnectionStep::StartOk(..) => "StartOk",
            ConnectionStep::SecureOk(..) => "SecureOk",
            ConnectionStep::Open(..) => "Open",
        }
    }

    pub(crate) fn into_connection_resolver(
        self,
    ) -> (PromiseResolver<Connection>, Option<Connection>) {
        match self {
            ConnectionStep::ProtocolHeader(resolver, connection, ..) => {
                (resolver, Some(connection))
            }
            ConnectionStep::StartOk(resolver, connection, ..) => (resolver, Some(connection)),
            ConnectionStep::SecureOk(resolver, connection, ..) => (resolver, Some(connection)),
            ConnectionStep::Open(resolver, connection, ..) => (resolver, Some(connection)),
        }
    }
}

impl fmt::Debug for ConnectionStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConnectionStep::{}", self.name())
    }
}
