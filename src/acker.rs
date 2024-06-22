use crate::{
    error_holder::ErrorHolder,
    internal_rpc::InternalRPCHandle,
    options::{BasicAckOptions, BasicNackOptions, BasicRejectOptions},
    protocol::{AMQPError, AMQPSoftError},
    types::{ChannelId, DeliveryTag},
    Error, Promise, PromiseResolver, Result,
};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone, Debug)]
pub struct Acker {
    channel_id: ChannelId,
    delivery_tag: DeliveryTag,
    internal_rpc: Option<InternalRPCHandle>,
    error: Option<ErrorHolder>,
    used: Arc<AtomicBool>,
}

impl Acker {
    pub(crate) fn new(
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        internal_rpc: Option<InternalRPCHandle>,
        error: Option<ErrorHolder>,
    ) -> Self {
        Self {
            channel_id,
            delivery_tag,
            internal_rpc,
            error,
            used: Arc::default(),
        }
    }

    pub async fn ack(&self, options: BasicAckOptions) -> Result<()> {
        self.rpc(|internal_rpc, resolver| {
            internal_rpc.basic_ack(
                self.channel_id,
                self.delivery_tag,
                options,
                resolver,
                self.error.clone(),
            )
        })
        .await
    }

    pub async fn nack(&self, options: BasicNackOptions) -> Result<()> {
        self.rpc(|internal_rpc, resolver| {
            internal_rpc.basic_nack(
                self.channel_id,
                self.delivery_tag,
                options,
                resolver,
                self.error.clone(),
            )
        })
        .await
    }

    pub async fn reject(&self, options: BasicRejectOptions) -> Result<()> {
        self.rpc(|internal_rpc, resolver| {
            internal_rpc.basic_reject(
                self.channel_id,
                self.delivery_tag,
                options,
                resolver,
                self.error.clone(),
            )
        })
        .await
    }

    async fn rpc<F: Fn(&InternalRPCHandle, PromiseResolver<()>)>(&self, f: F) -> Result<()> {
        if self.used.swap(true, Ordering::SeqCst) {
            return Err(Error::ProtocolError(AMQPError::new(
                AMQPSoftError::PRECONDITIONFAILED.into(),
                "Attempted to use an already used Acker".into(),
            )));
        }
        if let Some(error) = self.error.as_ref() {
            error.check()?;
        }
        if let Some(internal_rpc) = self.internal_rpc.as_ref() {
            let (promise, resolver) = Promise::new();
            f(internal_rpc, resolver);
            promise.await
        } else {
            Ok(())
        }
    }

    pub fn used(&self) -> bool {
        self.used.load(Ordering::SeqCst)
    }
}

impl PartialEq for Acker {
    fn eq(&self, other: &Acker) -> bool {
        self.channel_id == other.channel_id && self.delivery_tag == other.delivery_tag
    }
}
