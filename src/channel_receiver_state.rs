use crate::{
    types::{LongLongUInt, ShortString, ShortUInt},
    Result,
};
use std::collections::VecDeque;

#[derive(Debug, Default)]
pub(crate) struct ChannelReceiverStates(VecDeque<ChannelReceiverState>);

impl ChannelReceiverStates {
    #[cfg(test)]
    pub(crate) fn receiver_state(&self) -> ChannelReceiverState {
        self.0.front().unwrap().clone()
    }

    pub(crate) fn set_will_receive(&mut self, class_id: ShortUInt, delivery_cause: DeliveryCause) {
        self.0.push_back(ChannelReceiverState::WillReceiveContent(
            class_id,
            delivery_cause,
        ));
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_content_length<
        Handler: FnOnce(&DeliveryCause, bool) -> Result<()>,
        OnInvalidClass: FnOnce(String) -> Result<()>,
        OnError: FnOnce(String) -> Result<()>,
    >(
        &mut self,
        channel_id: u16,
        class_id: ShortUInt,
        length: LongLongUInt,
        handler: Handler,
        invalid_class_hanlder: OnInvalidClass,
        error_handler: OnError,
        confirm_mode: bool,
    ) -> Result<()> {
        if let Some(ChannelReceiverState::WillReceiveContent(expected_class_id, delivery_cause)) =
            self.0.pop_front()
        {
            if expected_class_id == class_id {
                let res = handler(&delivery_cause, confirm_mode);
                if length > 0 {
                    self.0.push_front(ChannelReceiverState::ReceivingContent(
                        delivery_cause,
                        length,
                    ));
                }
                res
            } else {
                invalid_class_hanlder(format!(
                    "content header frame with class id {} instead of {} received on channel {}",
                    class_id, expected_class_id, channel_id
                ))
            }
        } else {
            error_handler(format!(
                "unexpected content header frame received on channel {}",
                channel_id
            ))
        }
    }

    pub(crate) fn receive<
        Handler: FnOnce(&DeliveryCause, LongLongUInt, bool) -> Result<()>,
        OnError: FnOnce(String) -> Result<()>,
    >(
        &mut self,
        channel_id: u16,
        length: LongLongUInt,
        handler: Handler,
        error_handler: OnError,
        confirm_mode: bool,
    ) -> Result<()> {
        if let Some(ChannelReceiverState::ReceivingContent(delivery_cause, len)) =
            self.0.pop_front()
        {
            if let Some(remaining) = len.checked_sub(length) {
                let res = handler(&delivery_cause, remaining, confirm_mode);
                if remaining > 0 {
                    self.0.push_front(ChannelReceiverState::ReceivingContent(
                        delivery_cause,
                        remaining,
                    ));
                }
                res
            } else {
                error_handler(format!("unexpectedly large content body frame received on channel {} ({} ybtes, expected {} bytes)", channel_id, length, len))
            }
        } else {
            error_handler(format!(
                "unexpected content body frame received on channel {}",
                channel_id
            ))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChannelReceiverState {
    WillReceiveContent(ShortUInt, DeliveryCause),
    ReceivingContent(DeliveryCause, LongLongUInt),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeliveryCause {
    Consume(ShortString),
    Get,
    Return,
}
