use crate::{
    Error, Promise, PromiseResolver, channel::Reply, promise::Cancelable, types::ChannelId,
};
use amq_protocol::{
    frame::AMQPFrame,
    protocol::{AMQPClass, basic::AMQPMethod},
};
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};
use tracing::{Level, level_enabled, trace};

pub(crate) struct ExpectedReply(pub(crate) Reply, pub(crate) Box<dyn Cancelable + Send>);

impl fmt::Debug for ExpectedReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ExpectedReply").field(&self.0).finish()
    }
}

#[derive(Clone, Default)]
pub(crate) struct Frames(Arc<Mutex<Inner>>);

impl Frames {
    pub(crate) fn push(
        &self,
        channel_id: ChannelId,
        frame: AMQPFrame,
        resolver: PromiseResolver<()>,
        expected_reply: Option<ExpectedReply>,
    ) {
        self.lock_inner()
            .push(channel_id, frame, resolver, expected_reply);
    }

    pub(crate) fn push_frames(&self, frames: Vec<AMQPFrame>) -> Promise<()> {
        self.lock_inner().push_frames(frames)
    }

    pub(crate) fn retry(&self, frame: (AMQPFrame, Option<PromiseResolver<()>>)) {
        self.lock_inner().retry_frames.push_back(frame);
    }

    pub(crate) fn pop(&self, flow: bool) -> Option<(AMQPFrame, Option<PromiseResolver<()>>)> {
        self.lock_inner().pop(flow)
    }

    pub(crate) fn find_expected_reply<P: FnMut(&ExpectedReply) -> bool>(
        &self,
        channel_id: ChannelId,
        finder: P,
    ) -> Option<Reply> {
        self.lock_inner()
            .expected_replies
            .get_mut(&channel_id)
            .and_then(|replies| {
                replies
                    .iter()
                    .position(finder)
                    .and_then(|idx| replies.remove(idx))
            })
            .map(|t| t.0)
    }

    pub(crate) fn next_expected_close_ok_reply(
        &self,
        channel_id: ChannelId,
        error: Error,
    ) -> Option<Reply> {
        self.lock_inner()
            .next_expected_close_ok_reply(channel_id, error)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.lock_inner().has_pending()
    }

    pub(crate) fn drop_pending(&self, error: Error) {
        self.lock_inner().drop_pending(error);
    }

    pub(crate) fn take_expected_replies(
        &self,
        channel_id: ChannelId,
    ) -> Option<VecDeque<ExpectedReply>> {
        self.lock_inner().expected_replies.remove(&channel_id)
    }

    pub(crate) fn clear_expected_replies(&self, channel_id: ChannelId, error: Error) {
        if let Some(replies) = self.take_expected_replies(channel_id) {
            Self::cancel_expected_replies(replies, error)
        }
    }

    pub(crate) fn cancel_expected_replies(replies: VecDeque<ExpectedReply>, error: Error) {
        Inner::cancel_expected_replies(replies, error)
    }

    pub(crate) fn drop_frames_for_channel(&self, channel_id: ChannelId, error: Error) {
        self.lock_inner().drop_frames_for_channel(channel_id, error)
    }

    pub(crate) fn poison(&self) -> Option<Error> {
        self.lock_inner().poison.clone()
    }

    fn lock_inner(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Default)]
struct Inner {
    /* Header frames must follow basic.publish frames directly, otherwise RabbitMQ-server send us an UNEXPECTED_FRAME */
    /* After sending the Header frame, we need to send the associated Body frames before anything else for the same reason */
    publish_frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    retry_frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    low_prio_frames: VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
    expected_replies: HashMap<ChannelId, VecDeque<ExpectedReply>>,
    poison: Option<Error>,
}

impl fmt::Debug for Frames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Frames");
        if let Ok(inner) = self.0.try_lock() {
            debug.field("expected_replies", &inner.expected_replies);
        }
        debug.finish()
    }
}

impl Inner {
    fn push(
        &mut self,
        channel_id: ChannelId,
        frame: AMQPFrame,
        resolver: PromiseResolver<()>,
        expected_reply: Option<ExpectedReply>,
    ) {
        if self.check_poison(&resolver) {
            return;
        }

        self.frames.push_back((frame, Some(resolver)));
        if let Some(reply) = expected_reply {
            trace!(
                channel=%channel_id,
                expected_reply=?reply,
                "state is now waiting"
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

        if level_enabled!(Level::TRACE) {
            promise.set_marker("Frames".into());
        }

        if self.check_poison(&resolver) {
            return promise;
        }

        for frame in frames {
            self.low_prio_frames.push_back((frame, None));
        }
        if let Some(last_frame) = last_frame {
            self.low_prio_frames.push_back((last_frame, Some(resolver)));
        } else {
            resolver.resolve(());
        }
        promise
    }

    fn check_poison(&self, resolver: &PromiseResolver<()>) -> bool {
        if let Some(error) = self.poison.clone() {
            resolver.reject(error);
            return true;
        }
        false
    }

    fn pop(&mut self, flow: bool) -> Option<(AMQPFrame, Option<PromiseResolver<()>>)> {
        if let Some(frame) = self
            .retry_frames
            .pop_front()
            .or_else(|| self.publish_frames.pop_front())
            .or_else(|| self.frames.pop_front())
        {
            return Some(frame);
        }
        if flow {
            if let Some(frame) = self.low_prio_frames.pop_front() {
                // If the next frame is a header, that means we're a basic.publish
                // Header frame needs to follow directly the basic.publish frame, and Body frames
                // need to be sent just after those or the AMQP server will close the connection.
                // Push the header into publish_frames which is there to handle just that.
                if self
                    .low_prio_frames
                    .front()
                    .map(|(frame, _)| frame.is_header())
                    .unwrap_or(false)
                {
                    // Yes, this will always be Some() with a Header frame, but let's keep our unwrap() count low
                    if let Some(next_frame) = self.low_prio_frames.pop_front() {
                        self.publish_frames.push_back(next_frame);
                    }
                    while let Some(next_frame) = self.low_prio_frames.pop_front() {
                        match next_frame.0 {
                            AMQPFrame::Body(..) => {
                                self.publish_frames.push_back(next_frame);
                            }
                            _ => {
                                // We've exhausted Body frames for this publish, push back the next one and exit
                                self.low_prio_frames.push_front(next_frame);
                                break;
                            }
                        }
                    }
                }
                return Some(frame);
            }
        }
        None
    }

    fn has_pending(&self) -> bool {
        !(self.retry_frames.is_empty()
            && self.publish_frames.is_empty()
            && self.frames.is_empty()
            && self.low_prio_frames.is_empty())
    }

    fn drop_pending(&mut self, error: Error) {
        Self::drop_pending_frames(&mut self.retry_frames, error.clone());
        Self::drop_pending_frames(&mut self.publish_frames, error.clone());
        Self::drop_pending_frames(&mut self.frames, error.clone());
        Self::drop_pending_frames(&mut self.low_prio_frames, error.clone());
        for (_, replies) in self.expected_replies.drain() {
            Self::cancel_expected_replies(replies, error.clone());
        }
        self.poison = Some(error);
    }

    fn drop_pending_frames(
        frames: &mut VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
        error: Error,
    ) {
        for (frame, resolver) in std::mem::take(frames) {
            if let Some(resolver) = resolver {
                match frame {
                    AMQPFrame::Method(_, AMQPClass::Basic(AMQPMethod::Cancel(_))) => {
                        resolver.resolve(())
                    }
                    _ => resolver.reject(error.clone()),
                }
            }
        }
    }

    fn drop_frames_for_channel(&mut self, channel_id: ChannelId, error: Error) {
        Self::drop_pending_frames_for_channel(channel_id, &mut self.retry_frames, error.clone());
        Self::drop_pending_frames_for_channel(channel_id, &mut self.publish_frames, error.clone());
        Self::drop_pending_frames_for_channel(channel_id, &mut self.frames, error.clone());
        Self::drop_pending_frames_for_channel(channel_id, &mut self.low_prio_frames, error);
    }

    fn drop_pending_frames_for_channel(
        channel_id: ChannelId,
        frames: &mut VecDeque<(AMQPFrame, Option<PromiseResolver<()>>)>,
        error: Error,
    ) {
        use AMQPFrame::*;

        frames.retain(|(f, r)| match f {
            Method(id, _) | Header(id, _, _) | Body(id, _) | Heartbeat(id) if *id == channel_id => {
                if let Some(r) = r {
                    r.reject(error.clone());
                }
                false
            }
            _ => true,
        })
    }

    fn next_expected_close_ok_reply(
        &mut self,
        channel_id: ChannelId,
        error: Error,
    ) -> Option<Reply> {
        let expected_replies = self.expected_replies.get_mut(&channel_id)?;
        while let Some(reply) = expected_replies.pop_front() {
            match &reply.0 {
                Reply::ChannelCloseOk(_) => return Some(reply.0),
                Reply::BasicCancelOk(resolver) => resolver.resolve(()), // Channel close means consumer is canceled automatically
                _ => reply.1.cancel(error.clone()),
            }
        }
        None
    }

    fn cancel_expected_replies(replies: VecDeque<ExpectedReply>, error: Error) {
        for ExpectedReply(reply, cancel) in replies {
            match reply {
                Reply::BasicCancelOk(resolver) => resolver.resolve(()),
                _ => cancel.cancel(error.clone()),
            }
        }
    }
}
