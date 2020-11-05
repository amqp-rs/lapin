use crate::types::ShortString;

#[derive(Default)]
pub struct TopologyDefinition {
    pub channels: Vec<ChannelDefinition>,
    // FIXME: exchanges
}

#[derive(Default)]
pub struct ChannelDefinition {
    pub queues: Vec<QueueDefinition>,
}

#[derive(Default)]
pub struct QueueDefinition {
    pub name: ShortString,
    // FIXME: attributes
    // FIXME: bindings
    pub consumers: Vec<ConsumerDefinition>,
}

#[derive(Default)]
pub struct ConsumerDefinition {
    pub tag: ShortString,
}
