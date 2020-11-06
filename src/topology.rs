use crate::{
    options::QueueDeclareOptions,
    types::{FieldTable, ShortString},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TopologyDefinition {
    pub channels: Vec<ChannelDefinition>,
    // FIXME: exchanges
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ChannelDefinition {
    pub queues: Vec<QueueDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct QueueDefinition {
    pub name: ShortString,
    pub params: Option<(QueueDeclareOptions, FieldTable)>,
    pub bindings: Vec<BindingDefinition>,
    pub consumers: Vec<ConsumerDefinition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BindingDefinition {
    pub exchange: ShortString,
    pub routing_key: ShortString,
    pub arguments: FieldTable,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConsumerDefinition {
    pub tag: ShortString,
}
