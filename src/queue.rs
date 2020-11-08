use crate::{
    consumer::Consumer,
    message::BasicGetMessage,
    options::QueueDeclareOptions,
    topology::{BindingDefinition, ConsumerDefinition, QueueDefinition},
    types::{FieldTable, ShortString},
    BasicProperties, Error, PromiseResolver, Result,
};
use std::{borrow::Borrow, collections::HashMap, fmt, hash::Hash};

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
    consumers: HashMap<ShortString, Consumer>,
    current_get_message: Option<(BasicGetMessage, PromiseResolver<Option<BasicGetMessage>>)>,
}

impl fmt::Debug for QueueState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueState")
            .field("name", &self.definition.name)
            .field("consumers", &self.consumers)
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
            consumers: HashMap::new(),
            current_get_message: None,
        }
    }

    pub(crate) fn absorb(&mut self, other: QueueState) {
        self.definition.options = other.definition.options;
        self.definition.arguments = other.definition.arguments;
    }

    pub(crate) fn register_consumer(&mut self, consumer_tag: ShortString, consumer: Consumer) {
        self.consumers.insert(consumer_tag, consumer);
    }

    pub(crate) fn deregister_consumer<S: Hash + Eq + ?Sized>(
        &mut self,
        consumer_tag: &S,
    ) -> Result<()>
    where
        ShortString: Borrow<S>,
    {
        if let Some(consumer) = self.consumers.remove(consumer_tag) {
            consumer.cancel()?;
        }
        Ok(())
    }

    pub(crate) fn get_consumer<S: Hash + Eq + ?Sized>(
        &mut self,
        consumer_tag: &S,
    ) -> Option<&mut Consumer>
    where
        ShortString: Borrow<S>,
    {
        self.consumers.get_mut(consumer_tag.borrow())
    }

    pub(crate) fn cancel_consumers(&mut self) -> Result<()> {
        self.consumers
            .drain()
            .map(|(_, consumer)| consumer.cancel())
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn error_consumers(&mut self, error: Error) -> Result<()> {
        self.consumers
            .drain()
            .map(|(_, consumer)| consumer.set_error(error.clone()))
            .fold(Ok(()), Result::and)
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

    pub(crate) fn drop_prefetched_messages(&mut self) -> Result<()> {
        self.consumers
            .values()
            .map(Consumer::drop_prefetched_messages)
            .fold(Ok(()), Result::and)
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

    pub(crate) fn consumers_topology(&self) -> Vec<ConsumerDefinition> {
        self.consumers
            .values()
            .map(|c| ConsumerDefinition {
                tag: c.tag(),
                options: c.options(),
                arguments: c.arguments(),
                queue: self.name(),
            })
            .collect()
    }
}
