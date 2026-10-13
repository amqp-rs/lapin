use crate::{
    Promise, PromiseResolver, Result,
    error_holder::ErrorHolder,
    internal_rpc::InternalRPCHandle,
    killswitch::KillSwitch,
    options::{BasicAckOptions, BasicNackOptions, BasicRejectOptions},
    types::{ChannelId, DeliveryTag},
};

/// Handle for acknowledging, negatively acknowledging, or rejecting a delivery.
///
/// Obtained from the `acker` field of a [`crate::message::Delivery`] or by
/// dereferencing a delivery. Each `Acker` is single-use: once any of
/// [`ack`], [`nack`], or [`reject`] is called the handle is consumed and
/// subsequent calls return `Ok(false)`.
///
/// If the channel is closed or errored, all outstanding `Acker`s for that
/// channel are *poisoned* and [`usable`] returns `false`. An acknowledgement
/// already under way at that point fails with the error that ended the channel.
///
/// [`ack`]: Self::ack
/// [`nack`]: Self::nack
/// [`reject`]: Self::reject
/// [`usable`]: Self::usable
#[derive(Clone, Debug)]
pub struct Acker {
    channel_id: ChannelId,
    delivery_tag: DeliveryTag,
    internal_rpc: Option<InternalRPCHandle>,
    killswitch: KillSwitch,
    poison: ErrorHolder,
}

impl Acker {
    pub(crate) fn new(
        channel_id: ChannelId,
        delivery_tag: DeliveryTag,
        internal_rpc: Option<InternalRPCHandle>,
        poison: ErrorHolder,
    ) -> Self {
        Self {
            channel_id,
            delivery_tag,
            internal_rpc,
            killswitch: KillSwitch::default(),
            poison,
        }
    }

    /// Acknowledge the delivery.
    ///
    /// Signals to the server that the message has been successfully processed.
    /// Returns `Ok(true)` on success, `Ok(false)` if the acker is already used
    /// or poisoned.
    pub async fn ack(&self, options: BasicAckOptions) -> Result<bool> {
        self.rpc("basic.ack", |internal_rpc, resolver| {
            internal_rpc.basic_ack(
                self.channel_id,
                self.delivery_tag,
                options,
                resolver,
                self.poison.clone(),
            )
        })
        .await
    }

    /// Negatively acknowledge the delivery.
    ///
    /// Signals to the server that the message could not be processed. If
    /// [`BasicNackOptions::requeue`] is `true` the message is re-queued.
    /// Returns `Ok(true)` on success, `Ok(false)` if the acker is already used
    /// or poisoned.
    pub async fn nack(&self, options: BasicNackOptions) -> Result<bool> {
        self.rpc("basic.nack", |internal_rpc, resolver| {
            internal_rpc.basic_nack(
                self.channel_id,
                self.delivery_tag,
                options,
                resolver,
                self.poison.clone(),
            )
        })
        .await
    }

    /// Reject the delivery.
    ///
    /// Signals to the server that the message could not be processed. If
    /// [`BasicRejectOptions::requeue`] is `true` the message is re-queued;
    /// otherwise it is discarded or dead-lettered. Returns `Ok(true)` on
    /// success, `Ok(false)` if the acker is already used or poisoned.
    pub async fn reject(&self, options: BasicRejectOptions) -> Result<bool> {
        self.rpc("basic.reject", |internal_rpc, resolver| {
            internal_rpc.basic_reject(
                self.channel_id,
                self.delivery_tag,
                options,
                resolver,
                self.poison.clone(),
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
        if let Some(internal_rpc) = self.internal_rpc.as_ref() {
            let (promise, resolver) = Promise::new(marker);
            f(internal_rpc, resolver);
            promise.await?;
        }
        Ok(true)
    }

    /// True if our channel got closed or encountered an error
    #[must_use]
    pub fn poisoned(&self) -> bool {
        self.poison.is_set()
    }

    /// False if poisoned or already used
    #[must_use]
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
