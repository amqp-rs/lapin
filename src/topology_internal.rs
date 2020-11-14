use crate::{
    channel::Channel,
    consumer::Consumer,
    topology::{
        ChannelDefinition, ConsumerDefinition, ExchangeDefinition, QueueDefinition,
        TopologyDefinition,
    },
};

#[derive(Clone, Debug, Default)]
pub(crate) struct TopologyInternal {
    pub(crate) exchanges: Vec<ExchangeDefinition>,
    pub(crate) queues: Vec<QueueDefinition>,
    pub(crate) channels: Vec<ChannelDefinitionInternal>,
}

impl From<TopologyDefinition> for TopologyInternal {
    fn from(mut definition: TopologyDefinition) -> Self {
        Self {
            exchanges: definition.exchanges,
            queues: definition.queues,
            channels: definition.channels.drain(..).map(From::from).collect(),
        }
    }
}

impl From<TopologyInternal> for TopologyDefinition {
    fn from(mut internal: TopologyInternal) -> Self {
        Self {
            exchanges: internal.exchanges,
            queues: internal.queues,
            channels: internal.channels.drain(..).map(From::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChannelDefinitionInternal {
    pub(crate) channel: Option<Channel>,
    pub(crate) queues: Vec<QueueDefinition>,
    pub(crate) consumers: Vec<ConsumerDefinitionInternal>,
}

impl From<ChannelDefinition> for ChannelDefinitionInternal {
    fn from(mut definition: ChannelDefinition) -> Self {
        Self {
            channel: None,
            queues: definition.queues,
            consumers: definition.consumers.drain(..).map(From::from).collect(),
        }
    }
}

impl From<ChannelDefinitionInternal> for ChannelDefinition {
    fn from(mut internal: ChannelDefinitionInternal) -> Self {
        Self {
            queues: internal.queues,
            consumers: internal.consumers.drain(..).map(From::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConsumerDefinitionInternal {
    pub(crate) consumer: Option<Consumer>,
    pub(crate) definition: ConsumerDefinition,
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
