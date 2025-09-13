use crate::{
    Promise, PromiseResolver, Result,
    error_holder::ErrorHolder,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    options::{BasicAckOptions, BasicNackOptions, BasicRejectOptions},
    types::{ChannelId, DeliveryTag},
};

#[derive(Clone, Debug)]
pub struct Acker {
    channel_id: ChannelId,
    delivery_tag: DeliveryTag,
    internal_rpc: Option<InternalRPCHandle>,
    error: Option<ErrorHolder>,
    killswitch: KillSwitch,
    channel_killswitch: KillSwitch,
}

impl Acker {
    pub(crate) fn new(
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        internal_rpc: Option<InternalRPCHandle>,
        error: Option<ErrorHolder>,
        channel_killswitch: KillSwitch,
    ) -> Self {
        Self {
            channel_id,
            delivery_tag,
            internal_rpc,
            error,
            killswitch: KillSwitch::default(),
            channel_killswitch,
        }
    }

    pub async fn ack(&self, options: BasicAckOptions) -> Result<bool> {
        self.rpc("basic.ack", |internal_rpc, resolver| {
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

    pub async fn nack(&self, options: BasicNackOptions) -> Result<bool> {
        self.rpc("basic.nack", |internal_rpc, resolver| {
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

    pub async fn reject(&self, options: BasicRejectOptions) -> Result<bool> {
        self.rpc("basic.reject", |internal_rpc, resolver| {
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

    async fn rpc<F: Fn(&InternalRPCHandle, PromiseResolver<()>)>(
        &self,
        marker: &str,
        f: F,
    ) -> Result<bool> {
        if self.poisoned() || !self.killswitch.kill() {
            return Ok(false);
        }
        if let Some(error) = self.error.as_ref() {
            error.check()?;
        }
        if let Some(internal_rpc) = self.internal_rpc.as_ref() {
            let (promise, resolver) = Promise::new(marker);
            f(internal_rpc, resolver);
            promise.await?;
        }
        Ok(true)
    }

    /// True if our channel got closed or encountered an error
    pub fn poisoned(&self) -> bool {
        self.channel_killswitch.killed()
    }

    /// False if poisoned or already used
    pub fn usable(&self) -> bool {
        !self.poisoned() && !self.killswitch.killed()
    }

    pub(crate) fn invalidate(&self) {
        self.killswitch.kill();
    }
}

impl PartialEq for Acker {
    fn eq(&self, other: &Acker) -> bool {
        self.channel_id == other.channel_id && self.delivery_tag == other.delivery_tag
    }
}
