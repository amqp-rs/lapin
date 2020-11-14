use crate::{
    channel::Channel,
    consumer::Consumer,
    exchange::ExchangeKind,
    options::{BasicConsumeOptions, ExchangeDeclareOptions, QueueDeclareOptions},
    queue::Queue,
    types::{FieldTable, ShortString},
};
use serde::{Deserialize, Serialize};
use std::ops::Deref;

/// The current topology definition
///
/// This contains the list of exhanges, queues, bindings, channels and consumers
/// declared on the current Connection.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TopologyDefinition {
    /// The exchanges declared in this topology.
    #[serde(default)]
    pub exchanges: Vec<ExchangeDefinition>,
    /// The "global" (not exclusive) declared in this topology.
    #[serde(default)]
    pub queues: Vec<QueueDefinition>,
    /// The channels declares in this topology
    #[serde(default)]
    pub channels: Vec<ChannelDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExchangeDefinition {
    pub name: ShortString,
    pub kind: Option<ExchangeKind>,
    pub options: Option<ExchangeDeclareOptions>,
    pub arguments: Option<FieldTable>,
    #[serde(default)]
    pub bindings: Vec<BindingDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct QueueDefinition {
    pub name: ShortString,
    pub options: Option<QueueDeclareOptions>,
    pub arguments: Option<FieldTable>,
    #[serde(default)]
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
    #[serde(default)]
    pub arguments: FieldTable,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ChannelDefinition {
    /// Exclusive queues need to be declared in a Channel.
    /// This is the list of exclusive queues for this one.
    #[serde(default)]
    pub queues: Vec<QueueDefinition>,
    pub consumers: Vec<ConsumerDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConsumerDefinition {
    pub queue: ShortString,
    #[serde(default)]
    pub tag: ShortString,
    #[serde(default)]
    pub options: BasicConsumeOptions,
    #[serde(default)]
    pub arguments: FieldTable,
}

#[derive(Default)]
pub struct RestoredTopology {
    pub(crate) queues: Vec<Queue>,
    pub(crate) channels: Vec<RestoredChannel>,
}

impl RestoredTopology {
    pub fn queue(&self, index: usize) -> Queue {
        self.queues[index].clone()
    }

    pub fn channel(&self, index: usize) -> RestoredChannel {
        self.channels[index].clone()
    }
}

#[derive(Clone)]
pub struct RestoredChannel {
    pub(crate) channel: Channel,
    pub(crate) queues: Vec<Queue>,
    pub(crate) consumers: Vec<Consumer>,
}

impl Deref for RestoredChannel {
    type Target = Channel;

    fn deref(&self) -> &Self::Target {
        &self.channel
    }
}

impl RestoredChannel {
    pub(crate) fn new(channel: Channel) -> Self {
        Self {
            channel,
            queues: Vec::new(),
            consumers: Vec::new(),
        }
    }

    pub fn into_inner(self) -> Channel {
        self.channel
    }

    pub fn queue(&self, index: usize) -> Queue {
        self.queues[index].clone()
    }

    pub fn consumer(&self, index: usize) -> Consumer {
        self.consumers[index].clone()
    }
}
