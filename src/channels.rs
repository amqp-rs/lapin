use crate::{
    error_handler::ErrorHandler,
    executor::{Executor, ExecutorExt},
    frames::Frames,
    id_sequence::IdSequence,
    internal_rpc::InternalRPCHandle,
    protocol::{AMQPClass, AMQPError, AMQPHardError},
    waker::Waker,
    BasicProperties, Channel, ChannelState, Configuration, ConnectionState, ConnectionStatus,
    Error, Promise, Result,
};
use amq_protocol::frame::AMQPFrame;
use log::{debug, error, log_enabled, trace, Level::Trace};
use parking_lot::Mutex;
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Clone)]
pub(crate) struct Channels {
    inner: Arc<Mutex<Inner>>,
    connection_status: ConnectionStatus,
    internal_rpc: InternalRPCHandle,
    executor: Arc<dyn Executor>,
    frames: Frames,
    error_handler: ErrorHandler,
}

impl Channels {
    pub(crate) fn new(
        configuration: Configuration,
        connection_status: ConnectionStatus,
        waker: Waker,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn Executor>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::new(configuration, waker))),
            connection_status,
            internal_rpc,
            executor,
            frames,
            error_handler: ErrorHandler::default(),
        }
    }

    pub(crate) fn create(&self) -> Result<Channel> {
        self.inner.lock().create(
            self.connection_status.clone(),
            self.internal_rpc.clone(),
            self.frames.clone(),
            self.executor.clone(),
        )
    }

    pub(crate) fn create_zero(&self) {
        self.inner
            .lock()
            .create_channel(
                0,
                self.connection_status.clone(),
                self.internal_rpc.clone(),
                self.frames.clone(),
                self.executor.clone(),
            )
            .set_state(ChannelState::Connected);
    }

    pub(crate) fn get(&self, id: u16) -> Option<Channel> {
        self.inner.lock().channels.get(&id).cloned()
    }

    pub(crate) fn remove(&self, id: u16, error: Error) -> Result<()> {
        self.frames.clear_expected_replies(id, error);
        if self.inner.lock().channels.remove(&id).is_some() {
            Ok(())
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn receive_method(&self, id: u16, method: AMQPClass) -> Result<()> {
        if let Some(channel) = self.get(id) {
            channel.receive_method(method)
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn handle_content_header_frame(
        &self,
        id: u16,
        size: u64,
        properties: BasicProperties,
    ) -> Result<()> {
        if let Some(channel) = self.get(id) {
            channel.handle_content_header_frame(size, properties)
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn handle_body_frame(&self, id: u16, payload: Vec<u8>) -> Result<()> {
        if let Some(channel) = self.get(id) {
            channel.handle_body_frame(payload)
        } else {
            Err(Error::InvalidChannel(id))
        }
    }

    pub(crate) fn set_connection_closing(&self) {
        self.connection_status.set_state(ConnectionState::Closing);
        for channel in self.inner.lock().channels.values() {
            channel.set_state(ChannelState::Closing);
        }
    }

    pub(crate) fn set_connection_closed(&self, error: Error) -> Result<()> {
        self.connection_status.set_state(ConnectionState::Closed);
        self.inner
            .lock()
            .channels
            .drain()
            .map(|(id, channel)| {
                self.frames.clear_expected_replies(id, error.clone());
                channel.set_state(ChannelState::Closed);
                channel.error_publisher_confirms(error.clone());
                channel.cancel_consumers()
            })
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn set_connection_error(&self, error: Error) -> Result<()> {
        if let ConnectionState::Error = self.connection_status.state() {
            return Ok(());
        }

        error!("Connection error");
        self.connection_status.set_state(ConnectionState::Error);
        self.error_handler.on_error(error.clone());
        self.inner
            .lock()
            .channels
            .drain()
            .map(|(id, channel)| {
                self.frames.clear_expected_replies(id, error.clone());
                channel.set_state(ChannelState::Error);
                channel.error_publisher_confirms(error.clone());
                channel.error_consumers(error.clone())
            })
            .fold(Ok(()), Result::and)
    }

    pub(crate) fn flow(&self) -> bool {
        self.inner
            .lock()
            .channels
            .values()
            .all(|c| c.status().flow())
    }

    pub(crate) fn send_heartbeat(&self) -> Result<()> {
        debug!("send heartbeat");

        if let Some(channel0) = self.get(0) {
            let (promise, resolver) = Promise::new();

            if log_enabled!(Trace) {
                promise.set_marker("Heartbeat".into());
            }

            channel0.send_frame(AMQPFrame::Heartbeat(0), resolver, None)?;
            self.register_internal_promise(promise)
        } else {
            Err(Error::InvalidConnectionState(
                self.connection_status.state(),
            ))
        }
    }

    pub(crate) fn handle_frame(&self, f: AMQPFrame) -> Result<()> {
        if let Err(err) = self.do_handle_frame(f) {
            self.set_connection_error(err.clone())?;
            Err(err)
        } else {
            Ok(())
        }
    }

    fn do_handle_frame(&self, f: AMQPFrame) -> Result<()> {
        trace!("will handle frame: {:?}", f);
        match f {
            AMQPFrame::ProtocolHeader => {
                error!("error: the client should not receive a protocol header");
                return Err(Error::InvalidFrameReceived);
            }
            AMQPFrame::Method(channel_id, method) => {
                self.receive_method(channel_id, method)?;
            }
            AMQPFrame::Heartbeat(channel_id) => {
                if channel_id == 0 {
                    debug!("received heartbeat from server");
                } else {
                    error!("received invalid heartbeat on channel {}", channel_id);
                    let error = AMQPError::new(
                        AMQPHardError::FRAMEERROR.into(),
                        format!("heartbeat frame received on channel {}", channel_id).into(),
                    );
                    if let Some(channel0) = self.get(0) {
                        self.register_internal_promise(channel0.connection_close(
                            error.get_id(),
                            error.get_message().as_str(),
                            0,
                            0,
                        ))?;
                    }
                    return Err(Error::ProtocolError(error));
                }
            }
            AMQPFrame::Header(channel_id, _, header) => {
                self.handle_content_header_frame(channel_id, header.body_size, header.properties)?;
            }
            AMQPFrame::Body(channel_id, payload) => {
                self.handle_body_frame(channel_id, payload)?;
            }
        }
        Ok(())
    }

    pub(crate) fn set_error_handler<E: FnMut(Error) + Send + 'static>(&self, handler: E) {
        self.error_handler.set_handler(handler);
    }

    fn register_internal_promise(&self, promise: Promise<()>) -> Result<()> {
        self.executor
            .spawn_internal(promise, self.internal_rpc.clone())
    }
}

impl fmt::Debug for Channels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Channels");
        if let Some(inner) = self.inner.try_lock() {
            debug
                .field("channels", &inner.channels.values())
                .field("channel_id", &inner.channel_id)
                .field("configuration", &inner.configuration)
                .field("waker", &inner.waker);
        }
        debug
            .field("frames", &self.frames)
            .field("executor", &self.executor)
            .field("connection_status", &self.connection_status)
            .field("error_handler", &self.error_handler)
            .finish()
    }
}

struct Inner {
    channels: HashMap<u16, Channel>,
    channel_id: IdSequence<u16>,
    configuration: Configuration,
    waker: Waker,
}

impl Inner {
    fn new(configuration: Configuration, waker: Waker) -> Self {
        Self {
            channels: HashMap::default(),
            channel_id: IdSequence::new(false),
            configuration,
            waker,
        }
    }

    fn create_channel(
        &mut self,
        id: u16,
        connection_status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn Executor>,
    ) -> Channel {
        debug!("create channel with id {}", id);
        let channel = Channel::new(
            id,
            self.configuration.clone(),
            connection_status,
            self.waker.clone(),
            internal_rpc,
            frames,
            executor,
        );
        self.channels.insert(id, channel.clone());
        channel
    }

    fn create(
        &mut self,
        connection_status: ConnectionStatus,
        internal_rpc: InternalRPCHandle,
        frames: Frames,
        executor: Arc<dyn Executor>,
    ) -> Result<Channel> {
        debug!("create channel");
        self.channel_id.set_max(self.configuration.channel_max());
        let first_id = self.channel_id.next();
        let mut looped = false;
        let mut id = first_id;
        while !looped || id < first_id {
            if id == 1 {
                looped = true;
            }
            if !self.channels.contains_key(&id) {
                return Ok(self.create_channel(
                    id,
                    connection_status,
                    internal_rpc,
                    frames,
                    executor,
                ));
            }
            id = self.channel_id.next();
        }
        Err(Error::ChannelsLimitReached)
    }
}
