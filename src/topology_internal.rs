use crate::{
    channel::Channel,
    consumer::Consumer,
    message::BasicGetMessage,
    options::{BasicGetOptions, QueueDeclareOptions},
    topology::{
        BindingDefinition, ChannelDefinition, ConsumerDefinition, ExchangeDefinition,
        QueueDefinition, TopologyDefinition,
    },
    types::{FieldTable, ShortString},
    PromiseResolver,
};
use std::ops::Deref;

#[derive(Clone, Debug, Default)]
pub(crate) struct TopologyInternal {
    pub(crate) exchanges: Vec<ExchangeDefinition>,
    pub(crate) queues: Vec<QueueDefinitionInternal>,
    pub(crate) channels: Vec<ChannelDefinitionInternal>,
}

impl From<TopologyDefinition> for TopologyInternal {
    fn from(mut definition: TopologyDefinition) -> Self {
        Self {
            exchanges: definition.exchanges,
            queues: definition.queues.drain(..).map(From::from).collect(),
            channels: definition.channels.drain(..).map(From::from).collect(),
        }
    }
}

impl From<TopologyInternal> for TopologyDefinition {
    fn from(mut internal: TopologyInternal) -> Self {
        Self {
            exchanges: internal.exchanges,
            queues: internal.queues.drain(..).map(From::from).collect(),
            channels: internal.channels.drain(..).map(From::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ChannelDefinitionInternal {
    pub(crate) channel: Option<Channel>,
    pub(crate) queues: Vec<QueueDefinitionInternal>,
    pub(crate) consumers: Vec<ConsumerDefinitionInternal>,
}

impl From<ChannelDefinition> for ChannelDefinitionInternal {
    fn from(mut definition: ChannelDefinition) -> Self {
        Self {
            channel: None,
            queues: definition.queues.drain(..).map(From::from).collect(),
            consumers: definition.consumers.drain(..).map(From::from).collect(),
        }
    }
}

impl From<ChannelDefinitionInternal> for ChannelDefinition {
    fn from(mut internal: ChannelDefinitionInternal) -> Self {
        Self {
            queues: internal.queues.drain(..).map(From::from).collect(),
            consumers: internal.consumers.drain(..).map(From::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct QueueDefinitionInternal {
    definition: QueueDefinition,
    declared: bool,
}

impl QueueDefinitionInternal {
    pub(crate) fn declared(
        name: ShortString,
        options: QueueDeclareOptions,
        arguments: FieldTable,
    ) -> Self {
        Self {
            definition: QueueDefinition {
                name,
                options: Some(options),
                arguments: Some(arguments),
                bindings: Vec::new(),
            },
            declared: true,
        }
    }

    pub(crate) fn undeclared(name: ShortString) -> Self {
        Self {
            definition: QueueDefinition {
                name,
                options: None,
                arguments: None,
                bindings: Vec::new(),
            },
            declared: false,
        }
    }

    pub(crate) fn set_declared(&mut self, options: QueueDeclareOptions, arguments: FieldTable) {
        self.definition.options = Some(options);
        self.definition.arguments = Some(arguments);
        self.declared = true;
    }

    pub(crate) fn is_declared(&self) -> bool {
        self.declared
    }

    pub(crate) fn is_exclusive(&self) -> bool {
        self.definition.options.map_or(false, |o| o.exclusive)
    }

    pub(crate) fn register_binding(
        &mut self,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.definition.bindings.push(BindingDefinition {
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
        self.definition.bindings.retain(|binding| {
            binding.source.as_str() != source
                || binding.routing_key.as_str() != routing_key
                || &binding.arguments != arguments
        });
    }
}

impl Deref for QueueDefinitionInternal {
    type Target = QueueDefinition;

    fn deref(&self) -> &Self::Target {
        &self.definition
    }
}

impl From<QueueDefinition> for QueueDefinitionInternal {
    fn from(definition: QueueDefinition) -> Self {
        Self {
            definition,
            declared: true,
        }
    }
}

impl From<QueueDefinitionInternal> for QueueDefinition {
    fn from(internal: QueueDefinitionInternal) -> Self {
        internal.definition
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ConsumerDefinitionInternal {
    consumer: Option<Consumer>,
    definition: ConsumerDefinition,
}

impl ConsumerDefinitionInternal {
    pub(crate) fn new(consumer: Consumer) -> Self {
        let definition = ConsumerDefinition {
            tag: consumer.tag(),
            options: consumer.options(),
            arguments: consumer.arguments(),
            queue: consumer.queue(),
        };
        Self {
            consumer: Some(consumer),
            definition,
        }
    }

    pub(crate) fn original(&self) -> Option<Consumer> {
        self.consumer.clone()
    }
}

impl Deref for ConsumerDefinitionInternal {
    type Target = ConsumerDefinition;

    fn deref(&self) -> &Self::Target {
        &self.definition
    }
}

impl From<ConsumerDefinition> for ConsumerDefinitionInternal {
    fn from(definition: ConsumerDefinition) -> Self {
        Self {
            consumer: None,
            definition,
        }
    }
}

impl From<ConsumerDefinitionInternal> for ConsumerDefinition {
    fn from(internal: ConsumerDefinitionInternal) -> Self {
        internal.definition
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BasicGetDefinitionInternal {
    pub(crate) queue: ShortString,
    pub(crate) options: BasicGetOptions,
    pub(crate) resolver: PromiseResolver<Option<BasicGetMessage>>,
}
