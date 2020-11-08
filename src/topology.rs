use crate::{
    channel::Channel,
    consumer::Consumer,
    exchange::ExchangeKind,
    options::{BasicConsumeOptions, ExchangeDeclareOptions, QueueDeclareOptions},
    queue::Queue,
    types::{FieldTable, ShortString},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TopologyDefinition {
    pub exchanges: Vec<ExchangeDefinition>,
    pub queues: Vec<QueueDefinition>,
    pub channels: Vec<ChannelDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExchangeDefinition {
    pub name: ShortString,
    pub kind: Option<ExchangeKind>,
    pub options: Option<ExchangeDeclareOptions>,
    pub arguments: Option<FieldTable>,
    pub bindings: Vec<BindingDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct QueueDefinition {
    pub name: ShortString,
    pub options: Option<QueueDeclareOptions>,
    pub arguments: Option<FieldTable>,
    pub bindings: Vec<BindingDefinition>,
}

impl QueueDefinition {
    pub(crate) fn is_exclusive(&self) -> bool {
        self.options.map_or(false, |o| o.exclusive)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BindingDefinition {
    pub source: ShortString,
    pub routing_key: ShortString,
    pub arguments: FieldTable,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ChannelDefinition {
    pub queues: Vec<QueueDefinition>,
    pub consumers: Vec<ConsumerDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConsumerDefinition {
    pub queue: ShortString,
    pub tag: ShortString,
    pub options: BasicConsumeOptions,
    pub arguments: FieldTable,
}

#[derive(Default)]
pub struct RestoredTopology {
    pub queues: Vec<Queue>,
    pub channels: Vec<RestoredChannel>,
}

pub struct RestoredChannel {
    pub channel: Channel,
    pub queues: Vec<Queue>,
    pub consumers: Vec<Consumer>,
}

impl RestoredChannel {
    pub(crate) fn new(channel: Channel) -> Self {
        Self {
            channel,
            queues: Vec::new(),
            consumers: Vec::new(),
        }
    }
}
