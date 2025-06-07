use crate::{
    consumer::Consumer,
    exchange::ExchangeKind,
    options::{ExchangeDeclareOptions, QueueDeclareOptions},
    types::{FieldTable, ShortString},
};

/* FIXME: use this for connection recovery
/// The current topology definition
///
/// This contains the list of exhanges, queues, bindings, channels and consumers
/// declared on the current Connection.
#[derive(Clone, Debug, Default)]
pub(crate) struct TopologyDefinition {
    /// The channels declares in this topology
    pub(crate) channels: Vec<ChannelDefinition>,
}
*/

#[derive(Clone, Debug, Default)]
pub(crate) struct ChannelDefinition {
    pub(crate) exchanges: Vec<ExchangeDefinition>,
    pub(crate) queues: Vec<QueueDefinition>,
    pub(crate) consumers: Vec<Consumer>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BindingDefinition {
    pub(crate) source: ShortString,
    pub(crate) routing_key: ShortString,
    pub(crate) arguments: FieldTable,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ExchangeDefinition {
    pub(crate) name: ShortString,
    pub(crate) kind: Option<ExchangeKind>,
    pub(crate) options: Option<ExchangeDeclareOptions>,
    pub(crate) arguments: Option<FieldTable>,
    pub(crate) bindings: Vec<BindingDefinition>,
    pub(crate) is_declared: bool,
}

impl ExchangeDefinition {
    pub(crate) fn declared(
        name: ShortString,
        kind: ExchangeKind,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
    ) -> Self {
        Self {
            name,
            kind: Some(kind),
            options: Some(options),
            arguments: Some(arguments),
            bindings: Vec::new(),
            is_declared: true,
        }
    }

    pub(crate) fn undeclared(name: ShortString) -> Self {
        Self {
            name,
            kind: None,
            options: None,
            arguments: None,
            bindings: Vec::new(),
            is_declared: false,
        }
    }

    pub(crate) fn set_declared(
        &mut self,
        kind: ExchangeKind,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
    ) {
        self.kind = Some(kind);
        self.options = Some(options);
        self.arguments = Some(arguments);
        self.is_declared = true;
    }

    pub(crate) fn register_binding(
        &mut self,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.bindings.push(BindingDefinition {
            source,
            routing_key,
            arguments,
        });
    }

    pub(crate) fn deregister_binding(
        &mut self,
        source: &str,
        routing_key: &str,
        arguments: &FieldTable,
    ) {
        self.bindings.retain(|binding| {
            binding.source.as_str() != source
                || binding.routing_key.as_str() != routing_key
                || &binding.arguments != arguments
        });
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct QueueDefinition {
    pub(crate) name: ShortString,
    pub(crate) options: Option<QueueDeclareOptions>,
    pub(crate) arguments: Option<FieldTable>,
    pub(crate) bindings: Vec<BindingDefinition>,
    pub(crate) is_declared: bool,
}

impl QueueDefinition {
    pub(crate) fn declared(
        name: ShortString,
        options: QueueDeclareOptions,
        arguments: FieldTable,
    ) -> Self {
        Self {
            name,
            options: Some(options),
            arguments: Some(arguments),
            bindings: Vec::new(),
            is_declared: true,
        }
    }

    pub(crate) fn undeclared(name: ShortString) -> Self {
        Self {
            name,
            options: None,
            arguments: None,
            bindings: Vec::new(),
            is_declared: false,
        }
    }

    pub(crate) fn set_declared(&mut self, options: QueueDeclareOptions, arguments: FieldTable) {
        self.options = Some(options);
        self.arguments = Some(arguments);
        self.is_declared = true;
    }

    pub(crate) fn register_binding(
        &mut self,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.bindings.push(BindingDefinition {
            source,
            routing_key,
            arguments,
        });
    }

    pub(crate) fn deregister_binding(
        &mut self,
        source: &str,
        routing_key: &str,
        arguments: &FieldTable,
    ) {
        self.bindings.retain(|binding| {
            binding.source.as_str() != source
                || binding.routing_key.as_str() != routing_key
                || &binding.arguments != arguments
        });
    }
}
