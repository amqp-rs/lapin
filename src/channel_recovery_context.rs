use crate::{
    frames::{ExpectedReply, Frames},
    notifier::Notifier,
    Error,
};

use std::collections::VecDeque;

pub(crate) struct ChannelRecoveryContext {
    cause: Error,
    expected_replies: Option<VecDeque<ExpectedReply>>,
    notifier: Notifier,
}

impl ChannelRecoveryContext {
    pub(crate) fn new(cause: Error) -> Self {
        Self {
            cause,
            expected_replies: None,
            notifier: Notifier::default(),
        }
    }

    pub(crate) fn notifier(&self) -> Notifier {
        self.notifier.clone()
    }

    pub(crate) fn set_expected_replies(
        &mut self,
        expected_replies: Option<VecDeque<ExpectedReply>>,
    ) {
        self.expected_replies = expected_replies;
    }

    pub(crate) fn finalize_recovery(self) {
        self.notifier.notify_all();
        if let Some(replies) = self.expected_replies {
            Frames::cancel_expected_replies(replies, self.cause);
        }
    }
}
