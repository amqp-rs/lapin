use crate::{
    message::BasicGetMessage,
    options::QueueDeclareOptions,
    topology::{BindingDefinition, QueueDefinition},
    types::{FieldTable, ShortString},
    BasicProperties, PromiseResolver,
};
use std::{borrow::Borrow, fmt};

#[derive(Clone, Debug)]
pub struct Queue {
    name: ShortString,
    message_count: u32,
    consumer_count: u32,
}

impl Queue {
    pub fn name(&self) -> &ShortString {
        &self.name
    }

    pub fn message_count(&self) -> u32 {
        self.message_count
    }

    pub fn consumer_count(&self) -> u32 {
        self.consumer_count
    }
}

pub(crate) struct QueueState {
    definition: QueueDefinition,
    current_get_message: Option<(BasicGetMessage, PromiseResolver<Option<BasicGetMessage>>)>,
}

impl fmt::Debug for QueueState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueState")
            .field("name", &self.definition.name)
            .finish()
    }
}

impl Queue {
    pub(crate) fn new(name: ShortString, message_count: u32, consumer_count: u32) -> Self {
        Self {
            name,
            message_count,
            consumer_count,
        }
    }
}

impl Borrow<str> for Queue {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}

impl QueueState {
    pub(crate) fn new(
        name: ShortString,
        options: Option<QueueDeclareOptions>,
        arguments: Option<FieldTable>,
    ) -> Self {
        Self {
            definition: QueueDefinition {
                name,
                options,
                arguments,
                bindings: Vec::new(),
            },
            current_get_message: None,
        }
    }

    pub(crate) fn absorb(&mut self, other: QueueState) {
        self.definition.options = other.definition.options;
        self.definition.arguments = other.definition.arguments;
    }

    pub(crate) fn is_exclusive(&self) -> bool {
        self.definition.is_exclusive()
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
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.definition.bindings.retain(|binding| {
            binding.source != source
                || binding.routing_key != routing_key
                || binding.arguments != arguments
        });
    }

    pub(crate) fn name(&self) -> ShortString {
        self.definition.name.clone()
    }

    pub(crate) fn start_new_delivery(
        &mut self,
        delivery: BasicGetMessage,
        resolver: PromiseResolver<Option<BasicGetMessage>>,
    ) {
        self.current_get_message = Some((delivery, resolver));
    }

    pub(crate) fn set_delivery_properties(&mut self, properties: BasicProperties) {
        if let Some(delivery) = self.current_get_message.as_mut() {
            delivery.0.delivery.properties = properties;
        }
    }

    pub(crate) fn receive_delivery_content(&mut self, payload: Vec<u8>) {
        if let Some(delivery) = self.current_get_message.as_mut() {
            delivery.0.delivery.receive_content(payload);
        }
    }

    pub(crate) fn new_delivery_complete(&mut self) {
        if let Some((message, resolver)) = self.current_get_message.take() {
            resolver.swear(Ok(Some(message)));
        }
    }

    pub(crate) fn topology(&self) -> Option<QueueDefinition> {
        if self.is_exclusive() {
            Some(self.definition.clone())
        } else {
            None
        }
    }
}
