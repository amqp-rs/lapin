use crate::{
    exchange::ExchangeKind,
    options::{ExchangeDeclareOptions, QueueDeclareOptions},
    topology::{ExchangeDefinition, QueueDefinition},
    types::{FieldTable, ShortString},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone, Default)]
pub(crate) struct Registry(Arc<Mutex<Inner>>);

impl Registry {
    pub(crate) fn exchanges_topology(&self) -> Vec<ExchangeDefinition> {
        self.lock_inner().exchanges.values().cloned().collect()
    }

    pub(crate) fn queues_topology(&self) -> Vec<QueueDefinition> {
        self.lock_inner().queues.values().cloned().collect()
    }

    pub(crate) fn register_exchange(
        &self,
        name: ShortString,
        kind: ExchangeKind,
        options: ExchangeDeclareOptions,
        arguments: FieldTable,
    ) {
        let mut inner = self.lock_inner();
        if let Some(exchange) = inner.exchanges.get_mut(&name) {
            exchange.set_declared(kind, options, arguments);
        } else {
            inner.exchanges.insert(
                name.clone(),
                ExchangeDefinition::declared(name, kind, options, arguments),
            );
        }
    }

    pub(crate) fn deregister_exchange(&self, name: &str) {
        self.lock_inner().exchanges.remove(name);
    }

    pub(crate) fn register_exchange_binding(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.lock_inner()
            .exchanges
            .entry(destination.clone())
            .or_insert_with(|| ExchangeDefinition::undeclared(destination))
            .register_binding(source, routing_key, arguments);
    }

    pub(crate) fn deregister_exchange_binding(
        &self,
        destination: &str,
        source: &str,
        routing_key: &str,
        arguments: &FieldTable,
    ) {
        if let Some(destination) = self.lock_inner().exchanges.get_mut(destination) {
            destination.deregister_binding(source, routing_key, arguments);
        }
    }

    pub(crate) fn register_queue(
        &self,
        name: ShortString,
        options: QueueDeclareOptions,
        arguments: FieldTable,
    ) {
        let mut inner = self.lock_inner();
        if let Some(queue) = inner.queues.get_mut(&name) {
            queue.set_declared(options, arguments);
        } else {
            inner.queues.insert(
                name.clone(),
                QueueDefinition::declared(name, options, arguments),
            );
        }
    }

    pub(crate) fn deregister_queue(&self, name: &str) {
        self.lock_inner().queues.remove(name);
    }

    pub(crate) fn register_queue_binding(
        &self,
        destination: ShortString,
        source: ShortString,
        routing_key: ShortString,
        arguments: FieldTable,
    ) {
        self.lock_inner()
            .queues
            .entry(destination.clone())
            .or_insert_with(|| QueueDefinition::undeclared(destination))
            .register_binding(source, routing_key, arguments);
    }

    pub(crate) fn deregister_queue_binding(
        &self,
        destination: &str,
        source: &str,
        routing_key: &str,
        arguments: &FieldTable,
    ) {
        if let Some(destination) = self.lock_inner().queues.get_mut(destination) {
            destination.deregister_binding(source, routing_key, arguments);
        }
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Default)]
struct Inner {
    exchanges: HashMap<ShortString, ExchangeDefinition>,
    queues: HashMap<ShortString, QueueDefinition>,
}
