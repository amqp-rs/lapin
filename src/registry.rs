use crate::{
    exchange::ExchangeKind,
    options::{ExchangeDeclareOptions, QueueDeclareOptions},
    topology::{BindingDefinition, ExchangeDefinition, QueueDefinition},
    types::{FieldTable, ShortString},
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Default)]
pub(crate) struct Registry(Arc<Mutex<Inner>>);

impl Registry {
    pub(crate) fn exchanges_topology(&self) -> Vec<ExchangeDefinition> {
        self.0.lock().exchanges.values().cloned().collect()
    }

    pub(crate) fn queues_topology(&self) -> Vec<QueueDefinition> {
        self.0
            .lock()
            .queues
            .values()
            .filter(|q| q.options.map(|o| !o.exclusive).unwrap_or(false))
            .cloned()
            .collect()
    }

    pub(crate) fn register_exchange(
        &self,
        name: ShortString,
        kind: ExchangeKind,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
    ) {
        let mut inner = self.0.lock();
        if let Some(exchange) = inner.exchanges.get_mut(&name) {
            exchange.kind = Some(kind);
            exchange.options = Some(options);
            exchange.arguments = Some(arguments);
        } else {
            inner.exchanges.insert(
                name.clone(),
                ExchangeDefinition {
                    name,
                    kind: Some(kind),
                    options: Some(options),
                    arguments: Some(arguments),
                    bindings: Vec::new(),
                },
            );
        }
    }

    pub(crate) fn deregister_exchange(&self, name: ShortString) {
        self.0.lock().exchanges.remove(&name);
    }

    pub(crate) fn register_exchange_binding(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.0
            .lock()
            .exchanges
            .entry(destination.clone())
            .or_insert_with(|| ExchangeDefinition {
                name: destination,
                kind: None,
                options: None,
                arguments: None,
                bindings: Vec::new(),
            })
            .bindings
            .push(BindingDefinition {
                source,
                routing_key,
                arguments,
            });
    }

    pub(crate) fn deregister_exchange_binding(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        if let Some(destination) = self.0.lock().exchanges.get_mut(&destination) {
            destination.bindings.retain(|binding| {
                binding.source != source
                    || binding.routing_key != routing_key
                    || binding.arguments != arguments
            });
        }
    }

    pub(crate) fn register_queue(
        &self,
        name: ShortString,
        options: QueueDeclareOptions,
        arguments: FieldTable,
    ) {
        let mut inner = self.0.lock();
        if let Some(queue) = inner.queues.get_mut(&name) {
            queue.options = Some(options);
            queue.arguments = Some(arguments);
        } else {
            inner.queues.insert(
                name.clone(),
                QueueDefinition::new(name, Some(options), Some(arguments)),
            );
        }
    }

    pub(crate) fn deregister_queue(&self, name: ShortString) {
        self.0.lock().queues.remove(&name);
    }

    pub(crate) fn register_queue_binding(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.0
            .lock()
            .queues
            .entry(destination.clone())
            .or_insert_with(|| QueueDefinition {
                name: destination,
                options: None,
                arguments: None,
                bindings: Vec::new(),
            })
            .register_binding(source, routing_key, arguments);
    }

    pub(crate) fn deregister_queue_binding(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        if let Some(destination) = self.0.lock().queues.get_mut(&destination) {
            destination.deregister_binding(source, routing_key, arguments);
        }
    }
}

#[derive(Default)]
struct Inner {
    exchanges: HashMap<ShortString, ExchangeDefinition>,
    queues: HashMap<ShortString, QueueDefinition>,
}
