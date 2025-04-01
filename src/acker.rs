use crate::{
    error_holder::ErrorHolder,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    options::{BasicAckOptions, BasicNackOptions, BasicRejectOptions},
    protocol::{AMQPError, AMQPSoftError},
    types::{ChannelId, DeliveryTag},
    ErrorKind, Promise, PromiseResolver, Result,
};

#[derive(Clone, Debug)]
pub struct Acker {
    channel_id: ChannelId,
    delivery_tag: DeliveryTag,
    internal_rpc: Option<InternalRPCHandle>,
    error: Option<ErrorHolder>,
    killswitch: KillSwitch,
    channel_killswitch: Option<KillSwitch>,
}

impl Acker {
    pub(crate) fn new(
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        internal_rpc: Option<InternalRPCHandle>,
        error: Option<ErrorHolder>,
        channel_killswitch: Option<KillSwitch>,
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
        if !self.channel_usable() || !self.killswitch.kill() {
            return Err(ErrorKind::ProtocolError(AMQPError::new(
                AMQPSoftError::PRECONDITIONFAILED.into(),
                "Attempted to use a non usable Acker".into(),
            ))
            .into());
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

    pub(crate) fn channel_usable(&self) -> bool {
        self.channel_killswitch
            .as_ref()
            .map_or(true, |ks| !ks.killed())
    }

    pub fn usable(&self) -> bool {
        self.channel_usable() && !self.killswitch.killed()
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
