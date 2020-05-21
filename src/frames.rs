use crate::{channel::Reply, Error, Promise, PromiseResolver, Result};
use amq_protocol::frame::AMQPFrame;
use log::{log_enabled, trace, Level::Trace};
use parking_lot::Mutex;
use pinky_swear::Cancellable;
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
        f.debug_tuple("ExpectedReply").field(&self.0).finish()
    }
}

#[derive(Clone, Default)]
pub(crate) struct Frames {
    inner: Arc<Mutex<Inner>>,
}

impl Frames {
    pub(crate) fn push(
        &self,
        channel_id: u16,
        frame: AMQPFrame,
        resolver: PromiseResolver<()>,
        expected_reply: Option<ExpectedReply>,
    ) {
        self.inner
            .lock()
            .push(channel_id, frame, resolver, expected_reply);
    }

    pub(crate) async fn push_frames(&self, frames: Vec<AMQPFrame>) -> Result<()> {
        let promise = self.inner.lock().push_frames(frames);
        promise.await
    }

    pub(crate) fn retry(&self, frame: (AMQPFrame, Option<PromiseResolver<()>>)) {
        self.inner.lock().retry(frame);
    }

    pub(crate) fn pop(&self, flow: bool) -> Option<(AMQPFrame, Option<PromiseResolver<()>>)> {
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

struct Inner {
    /* Header frames must follow basic.publish frames directly, otherwise RabbitMQ-server send us an UNEXPECTED_FRAME */
    header_frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    priority_frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    low_prio_frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    expected_replies: HashMap<u16, VecDeque<ExpectedReply>>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            header_frames: VecDeque::default(),
            priority_frames: VecDeque::default(),
            frames: VecDeque::default(),
            low_prio_frames: VecDeque::default(),
            expected_replies: HashMap::default(),
        }
    }
}

impl fmt::Debug for Frames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Frames");
        if let Some(inner) = self.inner.try_lock() {
            debug.field("expected_replies", &inner.expected_replies);
        }
        debug.finish()
    }
}

impl Inner {
    fn push(
        &mut self,
        channel_id: u16,
        frame: AMQPFrame,
        resolver: PromiseResolver<()>,
        expected_reply: Option<ExpectedReply>,
    ) {
        self.frames.push_back((frame, Some(resolver)));
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

    fn push_frames(&mut self, mut frames: Vec<AMQPFrame>) -> Promise<()> {
        let (promise, resolver) = Promise::new();
        let last_frame = frames.pop();

        if log_enabled!(Trace) {
            promise.set_marker("Frames".into());
        }

        for frame in frames {
            self.low_prio_frames.push_back((frame, None));
        }
        if let Some(last_frame) = last_frame {
            self.low_prio_frames.push_back((last_frame, Some(resolver)));
        } else {
            resolver.swear(Ok(()));
        }
        promise
    }

    fn pop(&mut self, flow: bool) -> Option<(AMQPFrame, Option<PromiseResolver<()>>)> {
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
                    .map(|(frame, _)| frame.is_header())
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

    fn retry(&mut self, frame: (AMQPFrame, Option<PromiseResolver<()>>)) {
        if frame.0.is_header() {
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
        Self::drop_pending_frames(&mut self.header_frames, error.clone());
        Self::drop_pending_frames(&mut self.priority_frames, error.clone());
        Self::drop_pending_frames(&mut self.frames, error.clone());
        Self::drop_pending_frames(&mut self.low_prio_frames, error.clone());
        for (_, replies) in self.expected_replies.drain() {
            Self::cancel_expected_replies(replies, error.clone());
        }
    }

    fn drop_pending_frames(
        frames: &mut VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
        error: Error,
    ) {
        for (_, resolver) in std::mem::take(frames) {
            if let Some(resolver) = resolver {
                resolver.swear(Err(error.clone()));
            }
        }
    }

    fn clear_expected_replies(&mut self, channel_id: u16, error: Error) {
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
