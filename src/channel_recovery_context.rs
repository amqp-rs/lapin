use crate::{
    frames::{ExpectedReply, Frames},
    notifier::Notifier,
    topology_internal::ChannelDefinitionInternal,
    Error,
};

use std::collections::VecDeque;

pub(crate) struct ChannelRecoveryContext {
    cause: Error,
    topology: ChannelDefinitionInternal,
    expected_replies: Option<VecDeque<ExpectedReply>>,
    notifier: Notifier,
}

impl ChannelRecoveryContext {
    pub(crate) fn new(cause: Error, topology: ChannelDefinitionInternal) -> Self {
        let notifier = Notifier::default();
        Self {
            cause: cause.with_notifier(Some(notifier.clone())),
            topology,
            expected_replies: None,
            notifier,
        }
    }

    pub(crate) fn cause(&self) -> Error {
        self.cause.clone()
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
