use crate::{
    options::QueueDeclareOptions,
    types::{FieldTable, ShortString},
};

#[derive(Debug, Default)]
pub struct TopologyDefinition {
    pub channels: Vec<ChannelDefinition>,
    // FIXME: exchanges
}

#[derive(Debug, Default)]
pub struct ChannelDefinition {
    pub queues: Vec<QueueDefinition>,
}

#[derive(Debug, Default)]
pub struct QueueDefinition {
    pub name: ShortString,
    pub params: Option<(QueueDeclareOptions, FieldTable)>,
    // FIXME: bindings
    pub consumers: Vec<ConsumerDefinition>,
}

#[derive(Debug, Default)]
pub struct ConsumerDefinition {
    pub tag: ShortString,
}
