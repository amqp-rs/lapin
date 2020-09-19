use crate::{
    types::{ShortString, ShortUInt},
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

    pub(crate) fn set_will_receive(
        &mut self,
        class_id: ShortUInt,
        queue_name: Option<ShortString>,
        request_id_or_consumer_tag: Option<ShortString>,
    ) {
        self.0.push_back(ChannelReceiverState::WillReceiveContent(
            class_id,
            queue_name,
            request_id_or_consumer_tag,
        ));
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_content_length<
        Handler: FnOnce(&Option<ShortString>, &Option<ShortString>, bool),
        OnInvalidClass: FnOnce(String) -> Result<()>,
        OnError: FnOnce(String) -> Result<()>,
    >(
        &mut self,
        channel_id: u16,
        class_id: ShortUInt,
        length: usize,
        handler: Handler,
        invalid_class_hanlder: OnInvalidClass,
        error_handler: OnError,
        confirm_mode: bool,
    ) -> Result<()> {
        if let Some(ChannelReceiverState::WillReceiveContent(
            expected_class_id,
            queue_name,
            request_id_or_consumer_tag,
        )) = self.0.pop_front()
        {
            if expected_class_id == class_id {
                handler(&queue_name, &request_id_or_consumer_tag, confirm_mode);
                if length > 0 {
                    self.0.push_front(ChannelReceiverState::ReceivingContent(
                        queue_name,
                        request_id_or_consumer_tag,
                        length,
                    ));
                }
                Ok(())
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
        Handler: FnOnce(&Option<ShortString>, &Option<ShortString>, usize, bool),
        OnError: FnOnce(String) -> Result<()>,
    >(
        &mut self,
        channel_id: u16,
        length: usize,
        handler: Handler,
        error_handler: OnError,
        confirm_mode: bool,
    ) -> Result<()> {
        if let Some(ChannelReceiverState::ReceivingContent(
            queue_name,
            request_id_or_consumer_tag,
            len,
        )) = self.0.pop_front()
        {
            if let Some(remaining) = len.checked_sub(length) {
                handler(
                    &queue_name,
                    &request_id_or_consumer_tag,
                    remaining,
                    confirm_mode,
                );
                if remaining > 0 {
                    self.0.push_front(ChannelReceiverState::ReceivingContent(
                        queue_name,
                        request_id_or_consumer_tag,
                        remaining,
                    ));
                }
                Ok(())
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
    WillReceiveContent(ShortUInt, Option<ShortString>, Option<ShortString>),
    ReceivingContent(Option<ShortString>, Option<ShortString>, usize),
}
