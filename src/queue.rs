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
    creation_params: Option<(QueueDeclareOptions, FieldTable)>,
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
    name: ShortString,
    consumers: HashMap<ShortString, Consumer>,
    current_get_message: Option<(BasicGetMessage, PromiseResolver<Option<BasicGetMessage>>)>,
    creation_params: Option<(QueueDeclareOptions, FieldTable)>,
    bindings: Vec<Binding>,
}

impl fmt::Debug for QueueState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueueState")
            .field("name", &self.name)
            .field("consumers", &self.consumers)
            .finish()
    }
}

struct Binding {
    exchange: ShortString,
    routing_key: ShortString,
    arguments: FieldTable,
}

impl Queue {
    pub(crate) fn new(
        name: ShortString,
        message_count: u32,
        consumer_count: u32,
        creation_params: Option<(QueueDeclareOptions, FieldTable)>,
    ) -> Self {
        Self {
            name,
            message_count,
            consumer_count,
            creation_params,
        }
    }
}

impl Borrow<str> for Queue {
    fn borrow(&self) -> &str {
        self.name.as_str()
    }
}

impl QueueState {
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

    pub(crate) fn register_binding(
        &mut self,
        exchange: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.bindings.push(Binding {
            exchange,
            routing_key,
            arguments,
        });
    }

    pub(crate) fn deregister_binding(&mut self, exchange: ShortString, routing_key: ShortString) {
        self.bindings
            .retain(|binding| binding.exchange != exchange && binding.routing_key != routing_key);
    }

    pub(crate) fn name(&self) -> ShortString {
        self.name.clone()
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

    pub(crate) fn topology(&self) -> QueueDefinition {
        QueueDefinition {
            name: self.name.clone(),
            params: self.creation_params.clone(),
            bindings: self
                .bindings
                .iter()
                .map(|binding| BindingDefinition {
                    exchange: binding.exchange.clone(),
                    routing_key: binding.routing_key.clone(),
                    arguments: binding.arguments.clone(),
                })
                .collect(),
            consumers: self
                .consumers
                .keys()
                .map(|tag| ConsumerDefinition { tag: tag.clone() })
                .collect(),
        }
    }
}

impl From<Queue> for QueueState {
    fn from(queue: Queue) -> Self {
        Self {
            name: queue.name,
            consumers: HashMap::new(),
            current_get_message: None,
            creation_params: queue.creation_params,
            bindings: Vec::new(),
        }
    }
}
