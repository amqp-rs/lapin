use crate::{
    frames::{ExpectedReply, Frames},
    Error,
};

use std::collections::VecDeque;

pub(crate) struct ChannelRecoveryContext {
    cause: Error,
    expected_replies: Option<VecDeque<ExpectedReply>>,
}

impl ChannelRecoveryContext {
    pub(crate) fn new(cause: Error) -> Self {
        Self {
            cause,
            expected_replies: None,
        }
    }

    pub(crate) fn set_expected_replies(
        &mut self,
        expected_replies: Option<VecDeque<ExpectedReply>>,
    ) {
        self.expected_replies = expected_replies;
    }

    pub(crate) fn finalize_recovery(self) {
        if let Some(replies) = self.expected_replies {
            Frames::cancel_expected_replies(replies, self.cause);
        }
    }
}
