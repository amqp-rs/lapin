use crate::{
    exchange::ExchangeKind,
    options::{ExchangeDeclareOptions, QueueDeclareOptions},
    types::{FieldTable, ShortString},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TopologyDefinition {
    pub exchanges: Vec<ExchangeDefinition>,
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
    pub source: ShortString,
    pub routing_key: ShortString,
    pub arguments: FieldTable,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ConsumerDefinition {
    pub tag: ShortString,
}
