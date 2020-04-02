use crate::{
    channel::Reply,
    id_sequence::IdSequence,
    pinky_swear::{Cancellable, Pinky, PinkySwear},
    Error, Result,
};
use amq_protocol::frame::AMQPFrame;
use log::trace;
use parking_lot::Mutex;
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::Arc,
};

pub(crate) struct ExpectedReply(
    pub(crate) Reply,
    pub(crate) Box<dyn Cancellable<Error> + Send>,
);

impl fmt::Debug for ExpectedReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExpectedReply({:?})", self.0)
    }
}

pub(crate) type SendId = u64;

#[derive(Clone, Debug)]
pub(crate) enum Priority {
    CRITICAL,
    NORMAL,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::NORMAL
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Frames {
    inner: Arc<Mutex<Inner>>,
}

impl Frames {
    pub(crate) fn push(
        &self,
        channel_id: u16,
        priority: Priority,
        frame: AMQPFrame,
        pinky: Pinky<Result<()>>,
        expected_reply: Option<ExpectedReply>,
    ) {
        self.inner
            .lock()
            .push(channel_id, priority, frame, pinky, expected_reply);
    }

    pub(crate) fn push_frames(
        &self,
        channel_id: u16,
        frames: Vec<AMQPFrame>,
    ) -> PinkySwear<Result<()>> {
        self.inner.lock().push_frames(channel_id, frames)
    }

    pub(crate) fn retry(&self, frame: (SendId, AMQPFrame)) {
        self.inner.lock().retry(frame);
    }

    pub(crate) fn pop(&self, flow: bool) -> Option<(SendId, AMQPFrame)> {
        self.inner.lock().pop(flow)
    }

    pub(crate) fn next_expected_reply(&self, channel_id: u16) -> Option<Reply> {
        self.inner
            .lock()
            .expected_replies
            .get_mut(&channel_id)
            .and_then(|replies| replies.pop_front())
            .map(|t| t.0)
    }

    pub(crate) fn mark_sent(&self, send_id: SendId) {
        if let Some((_, pinky, _)) = self.inner.lock().outbox.remove(&send_id) {
            pinky.swear(Ok(()));
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.inner.lock().has_pending()
    }

    pub(crate) fn drop_pending(&self, error: Error) {
        self.inner.lock().drop_pending(error);
    }

    pub(crate) fn clear_expected_replies(&self, channel_id: u16, error: Error) {
        self.inner.lock().clear_expected_replies(channel_id, error);
    }
}

#[derive(Debug)]
struct Inner {
    /* Header frames must follow basic.publish frames directly, otherwise rabbitmq-server send us an UNEXPECTED_FRAME */
    header_frames: VecDeque<(SendId, AMQPFrame)>,
    priority_frames: VecDeque<(SendId, AMQPFrame)>,
    frames: VecDeque<(SendId, AMQPFrame)>,
    low_prio_frames: VecDeque<(SendId, AMQPFrame)>,
    expected_replies: HashMap<u16, VecDeque<ExpectedReply>>,
    outbox: HashMap<SendId, (u16, Pinky<Result<()>>, bool)>,
    send_id: IdSequence<SendId>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            header_frames: VecDeque::default(),
            priority_frames: VecDeque::default(),
            frames: VecDeque::default(),
            low_prio_frames: VecDeque::default(),
            expected_replies: HashMap::default(),
            outbox: HashMap::default(),
            send_id: IdSequence::new(false),
        }
    }
}

impl Inner {
    fn push(
        &mut self,
        channel_id: u16,
        priority: Priority,
        frame: AMQPFrame,
        pinky: Pinky<Result<()>>,
        expected_reply: Option<ExpectedReply>,
    ) {
        let send_id = self.send_id.next();

        self.outbox
            .insert(send_id, (channel_id, pinky, expected_reply.is_some()));

        match priority {
            Priority::CRITICAL => {
                self.priority_frames.push_front((send_id, frame));
            }
            Priority::NORMAL => {
                self.frames.push_back((send_id, frame));
            }
        };

        if let Some(reply) = expected_reply {
            trace!(
                "channel {} state is now waiting for {:?}",
                channel_id,
                reply
            );
            self.expected_replies
                .entry(channel_id)
                .or_default()
                .push_back(reply);
        }
    }

    fn push_frames(
        &mut self,
        channel_id: u16,
        mut frames: Vec<AMQPFrame>,
    ) -> PinkySwear<Result<()>> {
        let (promise, pinky) = PinkySwear::new();
        let last_frame = frames.pop();

        for frame in frames {
            self.low_prio_frames.push_back((0, frame));
        }
        if let Some(last_frame) = last_frame {
            let send_id = self.send_id.next();
            self.low_prio_frames.push_back((send_id, last_frame));
            self.outbox.insert(send_id, (channel_id, pinky, false));
        } else {
            pinky.swear(Ok(()));
        }

        promise
    }

    fn pop(&mut self, flow: bool) -> Option<(SendId, AMQPFrame)> {
        if let Some(frame) = self
            .header_frames
            .pop_front()
            .or_else(|| self.priority_frames.pop_front())
            .or_else(|| self.frames.pop_front())
        {
            return Some(frame);
        }
        if flow {
            if let Some(frame) = self.low_prio_frames.pop_front() {
                // If the next frame is a header, that means we're a basic.publish
                // Header frame needs to follow directly the basic.publish frame or
                // the AMQP server will close the connection.
                // Push the header into header_frames which is there to handle just that.
                if self
                    .low_prio_frames
                    .front()
                    .map(|(_, frame)| frame.is_header())
                    .unwrap_or(false)
                {
                    // Yes, this will always be Some(), but let's keep our unwrap() count low
                    if let Some(next_frame) = self.low_prio_frames.pop_front() {
                        self.header_frames.push_back(next_frame);
                    }
                }
                return Some(frame);
            }
        }
        None
    }

    fn retry(&mut self, frame: (SendId, AMQPFrame)) {
        if frame.1.is_header() {
            self.header_frames.push_front(frame);
        } else {
            self.priority_frames.push_back(frame);
        }
    }

    fn has_pending(&self) -> bool {
        !(self.header_frames.is_empty()
            && self.priority_frames.is_empty()
            && self.frames.is_empty()
            && self.low_prio_frames.is_empty())
    }

    fn drop_pending(&mut self, error: Error) {
        self.header_frames.clear();
        self.priority_frames.clear();
        self.frames.clear();
        self.low_prio_frames.clear();
        for (_, replies) in self.expected_replies.drain() {
            Self::cancel_expected_replies(replies, error.clone());
        }
        for (_, (_, pinky, _)) in self.outbox.drain() {
            pinky.swear(Ok(()));
        }
    }

    fn clear_expected_replies(&mut self, channel_id: u16, error: Error) {
        let mut outbox = HashMap::default();

        for (send_id, (chan_id, pinky, expects_reply)) in self.outbox.drain() {
            if chan_id == channel_id && expects_reply {
                pinky.swear(Err(error.clone()))
            } else {
                outbox.insert(send_id, (chan_id, pinky, expects_reply));
            }
        }

        self.outbox = outbox;

        if let Some(replies) = self.expected_replies.remove(&channel_id) {
            Self::cancel_expected_replies(replies, error);
        }
    }

    fn cancel_expected_replies(replies: VecDeque<ExpectedReply>, error: Error) {
        for ExpectedReply(_, cancel) in replies {
            cancel.cancel(error.clone());
        }
    }
}
