use amq_protocol_types::*;
use format::field::*;
use format::frame::*;
use connection::*;
use channel::*;
use queue::*;
use generated::*;
use error::*;
use callbacks::*;

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ChannelState {
    Initial,
    Connected,
    Closed,
    Error,
    SendingContent(usize),
    WillReceiveContent(String),
    ReceivingContent(String,usize),

    AwaitingChannelOpenOk,
    AwaitingChannelFlowOk,
    AwaitingChannelCloseOk,

    AwaitingAccessRequestOk,

    AwaitingExchangeDeclareOk,
    AwaitingExchangeDeleteOk,
    AwaitingExchangeBindOk,
    AwaitingExchangeUnbindOk,

    AwaitingQueueDeclareOk,
    AwaitingQueueBindOk(String),
    AwaitingQueuePurgeOk(String),
    AwaitingQueueDeleteOk(String),
    AwaitingQueueUnbindOk(String),

    AwaitingBasicQosOk(u32,u16,bool),
    AwaitingBasicConsumeOk(String, String, bool, bool, bool, bool),
    AwaitingBasicCancelOk,
    AwaitingBasicGetAnswer,
    AwaitingBasicRecoverOk,

    AwaitingTxSelectOk,
    AwaitingTxCommitOk,
    AwaitingTxRollbackOk,

    AwaitingConfirmSelectOk,
}

impl<'a> Connection<'a> {
    pub fn receive_method(&mut self, channel_id: u16, method: Class) -> Result<(), Error> {
        match method {

            Class::Channel(channel::Methods::OpenOk(m)) => {
                self.receive_channel_open_ok(channel_id, m)
            }
            Class::Channel(channel::Methods::Flow(m)) => self.receive_channel_flow(channel_id, m),
            Class::Channel(channel::Methods::FlowOk(m)) => {
                self.receive_channel_flow_ok(channel_id, m)
            }
            Class::Channel(channel::Methods::Close(m)) => self.receive_channel_close(channel_id, m),
            Class::Channel(channel::Methods::CloseOk(m)) => {
                self.receive_channel_close_ok(channel_id, m)
            }

            Class::Access(access::Methods::RequestOk(m)) => {
                self.receive_access_request_ok(channel_id, m)
            }

            Class::Exchange(exchange::Methods::DeclareOk(m)) => {
                self.receive_exchange_declare_ok(channel_id, m)
            }
            Class::Exchange(exchange::Methods::DeleteOk(m)) => {
                self.receive_exchange_delete_ok(channel_id, m)
            }
            Class::Exchange(exchange::Methods::Bind(m)) => {
                self.receive_exchange_bind(channel_id, m)
            }
            Class::Exchange(exchange::Methods::BindOk(m)) => {
                self.receive_exchange_bind_ok(channel_id, m)
            }
            Class::Exchange(exchange::Methods::Unbind(m)) => {
                self.receive_exchange_unbind(channel_id, m)
            }
            Class::Exchange(exchange::Methods::UnbindOk(m)) => {
                self.receive_exchange_unbind_ok(channel_id, m)
            }

            Class::Queue(queue::Methods::DeclareOk(m)) => {
                self.receive_queue_declare_ok(channel_id, m)
            }
            Class::Queue(queue::Methods::BindOk(m)) => self.receive_queue_bind_ok(channel_id, m),
            Class::Queue(queue::Methods::PurgeOk(m)) => self.receive_queue_purge_ok(channel_id, m),
            Class::Queue(queue::Methods::DeleteOk(m)) => {
                self.receive_queue_delete_ok(channel_id, m)
            }
            Class::Queue(queue::Methods::UnbindOk(m)) => {
                self.receive_queue_unbind_ok(channel_id, m)
            }

            Class::Basic(basic::Methods::QosOk(m)) => self.receive_basic_qos_ok(channel_id, m),
            Class::Basic(basic::Methods::ConsumeOk(m)) => {
                self.receive_basic_consume_ok(channel_id, m)
            }
            Class::Basic(basic::Methods::CancelOk(m)) => {
                self.receive_basic_cancel_ok(channel_id, m)
            }
            Class::Basic(basic::Methods::Return(m)) => {
                self.receive_basic_amqp_return(channel_id, m)
            }
            Class::Basic(basic::Methods::Deliver(m)) => self.receive_basic_deliver(channel_id, m),
            Class::Basic(basic::Methods::GetOk(m)) => self.receive_basic_get_ok(channel_id, m),
            Class::Basic(basic::Methods::GetEmpty(m)) => {
                self.receive_basic_get_empty(channel_id, m)
            }
            Class::Basic(basic::Methods::RecoverOk(m)) => {
                self.receive_basic_recover_ok(channel_id, m)
            }

            Class::Tx(tx::Methods::SelectOk(m)) => self.receive_tx_select_ok(channel_id, m),
            Class::Tx(tx::Methods::CommitOk(m)) => self.receive_tx_commit_ok(channel_id, m),
            Class::Tx(tx::Methods::RollbackOk(m)) => self.receive_tx_rollback_ok(channel_id, m),

            Class::Confirm(confirm::Methods::SelectOk(m)) => {
                self.receive_confirm_select_ok(channel_id, m)
            }

            m => {
                println!("the client should not receive this method: {:?}", m);
                return Err(Error::InvalidState);
            }
        }
    }

    pub fn channel_open(&mut self,
                        _channel_id: u16,
                        out_of_band: ShortString)
                        -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.check_state(_channel_id, ChannelState::Initial).unwrap_or(false) {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(Error::InvalidState);
        }

        let method =
            Class::Channel(channel::Methods::Open(channel::Open { out_of_band: out_of_band }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.set_channel_state(_channel_id, ChannelState::AwaitingChannelOpenOk);
        })
    }

    pub fn receive_channel_open_ok(&mut self,
                                   _channel_id: u16,
                                   method: channel::OpenOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingChannelOpenOk => {
                self.set_channel_state(_channel_id, ChannelState::Connected);
            }
            _ => {
                self.set_channel_state(_channel_id, ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Channel.OpenOk, ignoring packet");


        Ok(())
    }

    pub fn channel_flow(&mut self, _channel_id: u16, active: Boolean) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Channel(channel::Methods::Flow(channel::Flow { active: active }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                //FIXME: we might still be receiving content here
                c.state = ChannelState::AwaitingChannelFlowOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_channel_flow(&mut self,
                                _channel_id: u16,
                                method: channel::Flow)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Connected | ChannelState::SendingContent(_) => {}
            ChannelState::Error  |
            ChannelState::Closed |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        self.channels.get_mut(&_channel_id).map(|c| c.send_flow = method.active);
        self.channel_flow_ok(_channel_id, method.active)
    }

    pub fn channel_flow_ok(&mut self, _channel_id: u16, active: Boolean) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Connected  | ChannelState::SendingContent(_) => {}
            ChannelState::Error  |
            ChannelState::Closed |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        let method = Class::Channel(channel::Methods::FlowOk(channel::FlowOk { active: active }));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn receive_channel_flow_ok(&mut self,
                                   _channel_id: u16,
                                   method: channel::FlowOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            //FIXME: we might still be receiving content here
            ChannelState::AwaitingChannelFlowOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.receive_flow = method.active);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        Ok(())
    }

    pub fn channel_close(&mut self,
                         _channel_id: u16,
                         reply_code: ShortUInt,
                         reply_text: ShortString,
                         class_id: ShortUInt,
                         method_id: ShortUInt)
                         -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.check_state(_channel_id, ChannelState::Connected).unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Channel(channel::Methods::Close(channel::Close {
            reply_code: reply_code,
            reply_text: reply_text,
            class_id: class_id,
            method_id: method_id,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingChannelCloseOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_channel_close(&mut self,
                                 _channel_id: u16,
                                 method: channel::Close)
                                 -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Error => {
                return Err(Error::InvalidState);
            }
            _ => {}
        }

        //FIXME: log the error if there is one
        //FIXME: handle reply codes

        self.set_channel_state(_channel_id, ChannelState::Closed);
        self.channel_close_ok(_channel_id)
    }

    pub fn channel_close_ok(&mut self, _channel_id: u16) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if self.check_state(_channel_id, ChannelState::Initial).unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Channel(channel::Methods::CloseOk(channel::CloseOk {}));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn receive_channel_close_ok(&mut self,
                                    _channel_id: u16,
                                    method: channel::CloseOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::AwaitingChannelCloseOk => {
                self.set_channel_state(_channel_id, ChannelState::Closed);
                Ok(())
            }
            _ => {
                self.set_channel_state(_channel_id, ChannelState::Error);
                Err(Error::InvalidState)
            }
        }
    }

    pub fn access_request(&mut self,
                          _channel_id: u16,
                          realm: ShortString,
                          exclusive: Boolean,
                          passive: Boolean,
                          active: Boolean,
                          write: Boolean,
                          read: Boolean)
                          -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Access(access::Methods::Request(access::Request {
            realm: realm,
            exclusive: exclusive,
            passive: passive,
            active: active,
            write: write,
            read: read,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingAccessRequestOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }


    pub fn receive_access_request_ok(&mut self,
                                     _channel_id: u16,
                                     method: access::RequestOk)
                                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingAccessRequestOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Access.RequestOk, ignoring packet");


        Ok(())
    }

    pub fn exchange_declare(&mut self,
                            _channel_id: u16,
                            ticket: ShortUInt,
                            exchange: ShortString,
                            amqp_type: ShortString,
                            passive: Boolean,
                            durable: Boolean,
                            auto_delete: Boolean,
                            internal: Boolean,
                            nowait: Boolean,
                            arguments: FieldTable)
                            -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Exchange(exchange::Methods::Declare(exchange::Declare {
            ticket: ticket,
            exchange: exchange,
            amqp_type: amqp_type,
            passive: passive,
            durable: durable,
            auto_delete: auto_delete,
            internal: internal,
            nowait: nowait,
            arguments: arguments,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingExchangeDeclareOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_exchange_declare_ok(&mut self,
                                       _channel_id: u16,
                                       method: exchange::DeclareOk)
                                       -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingExchangeDeclareOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Exchange.DeclareOk, ignoring packet");


        Ok(())
    }

    pub fn exchange_delete(&mut self,
                           _channel_id: u16,
                           ticket: ShortUInt,
                           exchange: ShortString,
                           if_unused: Boolean,
                           nowait: Boolean)
                           -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Exchange(exchange::Methods::Delete(exchange::Delete {
            ticket: ticket,
            exchange: exchange,
            if_unused: if_unused,
            nowait: nowait,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingExchangeDeleteOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_exchange_delete_ok(&mut self,
                                      _channel_id: u16,
                                      method: exchange::DeleteOk)
                                      -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingExchangeDeleteOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Exchange.DeleteOk, ignoring packet");


        Ok(())
    }

    pub fn exchange_bind(&mut self,
                         _channel_id: u16,
                         ticket: ShortUInt,
                         destination: ShortString,
                         source: ShortString,
                         routing_key: ShortString,
                         nowait: Boolean,
                         arguments: FieldTable)
                         -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Exchange(exchange::Methods::Bind(exchange::Bind {
            ticket: ticket,
            destination: destination,
            source: source,
            routing_key: routing_key,
            nowait: nowait,
            arguments: arguments,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingExchangeBindOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_exchange_bind(&mut self,
                                 _channel_id: u16,
                                 method: exchange::Bind)
                                 -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Exchange.Bind, ignoring packet");


        Ok(())
    }

    pub fn exchange_bind_ok(&mut self, _channel_id: u16) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Exchange(exchange::Methods::BindOk(exchange::BindOk {}));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn receive_exchange_bind_ok(&mut self,
                                    _channel_id: u16,
                                    method: exchange::BindOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingExchangeBindOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Exchange.BindOk, ignoring packet");


        Ok(())
    }

    pub fn exchange_unbind(&mut self,
                           _channel_id: u16,
                           ticket: ShortUInt,
                           destination: ShortString,
                           source: ShortString,
                           routing_key: ShortString,
                           nowait: Boolean,
                           arguments: FieldTable)
                           -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Exchange(exchange::Methods::Unbind(exchange::Unbind {
            ticket: ticket,
            destination: destination,
            source: source,
            routing_key: routing_key,
            nowait: nowait,
            arguments: arguments,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingExchangeUnbindOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_exchange_unbind(&mut self,
                                   _channel_id: u16,
                                   method: exchange::Unbind)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Exchange.Unbind, ignoring packet");


        Ok(())
    }

    pub fn exchange_unbind_ok(&mut self, _channel_id: u16) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Exchange(exchange::Methods::UnbindOk(exchange::UnbindOk {}));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn receive_exchange_unbind_ok(&mut self,
                                      _channel_id: u16,
                                      method: exchange::UnbindOk)
                                      -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingExchangeUnbindOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Exchange.UnbindOk, ignoring packet");


        Ok(())
    }



    pub fn queue_declare(&mut self,
                         _channel_id: u16,
                         ticket: ShortUInt,
                         queue: ShortString,
                         passive: Boolean,
                         durable: Boolean,
                         exclusive: Boolean,
                         auto_delete: Boolean,
                         nowait: Boolean,
                         arguments: FieldTable)
                         -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.check_state(_channel_id, ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Queue(queue::Methods::Declare(queue::Declare {
            ticket: ticket,
            queue: queue.clone(),
            passive: passive,
            durable: durable,
            exclusive: exclusive,
            auto_delete: auto_delete,
            nowait: nowait,
            arguments: arguments,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                let q  = Queue::new(queue.clone(), passive, durable, exclusive, auto_delete);
                c.queues.insert(queue.clone(), q);
                //FIXME: when we set passive, the server might return an error if the queue exists
                c.state = ChannelState::AwaitingQueueDeclareOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_queue_declare_ok(&mut self,
                                    _channel_id: u16,
                                    method: queue::DeclareOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                self.set_channel_state(_channel_id, ChannelState::Error);
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingQueueDeclareOk => {
                self.channels.get_mut(&_channel_id).map(|c| {
                  c.queues.get_mut(&method.queue).map(|q| {
                    q.message_count  = method.message_count;
                    q.consumer_count = method.consumer_count;
                    q.created = true;
                  });
                });
                self.set_channel_state(_channel_id, ChannelState::Connected);
            }
            _ => {
                self.set_channel_state(_channel_id, ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Queue.DeclareOk, ignoring packet");


        Ok(())
    }

    pub fn queue_bind(&mut self,
                      _channel_id: u16,
                      ticket: ShortUInt,
                      queue: ShortString,
                      exchange: ShortString,
                      routing_key: ShortString,
                      nowait: Boolean,
                      arguments: FieldTable)
                      -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Queue(queue::Methods::Bind(queue::Bind {
            ticket: ticket,
            queue: queue.clone(),
            exchange: exchange.clone(),
            routing_key: routing_key.clone(),
            nowait: nowait,
            arguments: arguments,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                let key = format!("{}_{}", &exchange, &routing_key);
                c.state = ChannelState::AwaitingQueueBindOk(key.clone());
                c.queues.get_mut(&queue).map(|q| {
                  q.bindings.insert(key, Binding::new(exchange, routing_key, nowait)
                  );
                });
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_queue_bind_ok(&mut self,
                                 _channel_id: u16,
                                 method: queue::BindOk)
                                 -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingQueueBindOk(key) => {
                self.channels.get_mut(&_channel_id).map(|c| {
                    c.queues.iter_mut().map(|(_, ref mut q)| {
                      q.bindings.get_mut(&key).map(|b| b.active = true);
                    });
                });
                self.set_channel_state(_channel_id, ChannelState::Connected);
            }
            _ => {
                self.set_channel_state(_channel_id, ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        Ok(())
    }

    pub fn queue_purge(&mut self,
                       _channel_id: u16,
                       ticket: ShortUInt,
                       queue: ShortString,
                       nowait: Boolean)
                       -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Queue(queue::Methods::Purge(queue::Purge {
            ticket: ticket,
            queue: queue.clone(),
            nowait: nowait,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingQueuePurgeOk(queue.clone());
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_queue_purge_ok(&mut self,
                                  _channel_id: u16,
                                  method: queue::PurgeOk)
                                  -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingQueuePurgeOk(_) => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        Ok(())
    }

    pub fn queue_delete(&mut self,
                        _channel_id: u16,
                        ticket: ShortUInt,
                        queue: ShortString,
                        if_unused: Boolean,
                        if_empty: Boolean,
                        nowait: Boolean)
                        -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Queue(queue::Methods::Delete(queue::Delete {
            ticket: ticket,
            queue: queue.clone(),
            if_unused: if_unused,
            if_empty: if_empty,
            nowait: nowait,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingQueueDeleteOk(queue);
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_queue_delete_ok(&mut self,
                                   _channel_id: u16,
                                   method: queue::DeleteOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingQueueDeleteOk(key) => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
                self.channels.get_mut(&_channel_id).map(|c| c.queues.remove(&key));
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        Ok(())
    }

    pub fn queue_unbind(&mut self,
                        _channel_id: u16,
                        ticket: ShortUInt,
                        queue: ShortString,
                        exchange: ShortString,
                        routing_key: ShortString,
                        arguments: FieldTable)
                        -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Queue(queue::Methods::Unbind(queue::Unbind {
            ticket: ticket,
            queue: queue,
            exchange: exchange.clone(),
            routing_key: routing_key.clone(),
            arguments: arguments,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                let key = format!("{}_{}", &exchange, &routing_key);
                c.state = ChannelState::AwaitingQueueUnbindOk(key);
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_queue_unbind_ok(&mut self,
                                   _channel_id: u16,
                                   method: queue::UnbindOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingQueueUnbindOk(key) => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
                self.channels.get_mut(&_channel_id).map(|c| {
                    c.queues.iter_mut().map(|(_, ref mut q)| {
                      q.bindings.remove(&key);
                    });
                });
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        Ok(())
    }

    pub fn basic_qos(&mut self,
                     _channel_id: u16,
                     prefetch_size: LongUInt,
                     prefetch_count: ShortUInt,
                     global: Boolean)
                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Qos(basic::Qos {
            prefetch_size: prefetch_size,
            prefetch_count: prefetch_count,
            global: global,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingBasicQosOk(prefetch_size, prefetch_count, global);
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_basic_qos_ok(&mut self,
                                _channel_id: u16,
                                method: basic::QosOk)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingBasicQosOk(prefetch_size, prefetch_count, global) => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
                if global {
                  self.prefetch_size  = prefetch_size;
                  self.prefetch_count = prefetch_count;
                } else {
                  self.channels.get_mut(&_channel_id).map(|c| {
                    c.prefetch_size  = prefetch_size;
                    c.prefetch_count = prefetch_count;
                  });
                }
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        Ok(())
    }

    pub fn basic_consume<T>(&mut self,
                         callback: T,
                         _channel_id: u16,
                         ticket: ShortUInt,
                         queue: ShortString,
                         consumer_tag: ShortString,
                         no_local: Boolean,
                         no_ack: Boolean,
                         exclusive: Boolean,
                         nowait: Boolean,
                         arguments: FieldTable)
                         -> Result<(), Error>
                         where T: BasicConsumer + Clone + Send +'a {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Consume(basic::Consume {
            ticket: ticket,
            queue: queue.clone(),
            consumer_tag: consumer_tag.clone(),
            no_local: no_local,
            no_ack: no_ack,
            exclusive: exclusive,
            nowait: nowait,
            arguments: arguments,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.queues.get_mut(&queue).map(|q| q.callback_holder = Some(Box::new(callback)));
                c.state = ChannelState::AwaitingBasicConsumeOk(
                  queue, consumer_tag, no_local, no_ack, exclusive, nowait
                );
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_basic_consume_ok(&mut self,
                                    _channel_id: u16,
                                    method: basic::ConsumeOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingBasicConsumeOk(queue, tag, no_local, no_ack, exclusive, nowait) => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
                self.channels.get_mut(&_channel_id).map(|c| {
                  c.queues.get_mut(&queue).map(|q| {
                    let callback = q.callback_holder.take();
                    q.consumers.insert(
                      method.consumer_tag.clone(),
                      Consumer {
                        tag:       method.consumer_tag.clone(),
                        no_local:  no_local,
                        no_ack:    no_ack,
                        exclusive: exclusive,
                        nowait:    nowait,
                        callback:  callback.unwrap(),
                      }
                    )
                  })
                });
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        Ok(())
    }

    pub fn basic_cancel(&mut self,
                        _channel_id: u16,
                        consumer_tag: ShortString,
                        nowait: Boolean)
                        -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Cancel(basic::Cancel {
            consumer_tag: consumer_tag,
            nowait: nowait,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingBasicCancelOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_basic_cancel_ok(&mut self,
                                   _channel_id: u16,
                                   method: basic::CancelOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            //FIXME: we can receive messages while waiting for a cancel-ok
            ChannelState::AwaitingBasicCancelOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
                self.channels.get_mut(&_channel_id).map(|c| {
                  c.queues.iter_mut().map(|(_, ref mut q)| {
                    q.consumers.remove(&method.consumer_tag);
                  });
                });
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Basic.CancelOk, ignoring packet");


        Ok(())
    }

    pub fn basic_publish(&mut self,
                         _channel_id: u16,
                         ticket: ShortUInt,
                         exchange: ShortString,
                         routing_key: ShortString,
                         mandatory: Boolean,
                         immediate: Boolean)
                         -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Publish(basic::Publish {
            ticket: ticket,
            exchange: exchange,
            routing_key: routing_key,
            mandatory: mandatory,
            immediate: immediate,
        }));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn receive_basic_amqp_return(&mut self,
                                     _channel_id: u16,
                                     method: basic::Return)
                                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Basic.Return, ignoring packet");
        Ok(())
    }

    pub fn receive_basic_deliver(&mut self,
                                 _channel_id: u16,
                                 method: basic::Deliver)
                                 -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);

                return Err(Error::InvalidState);
            }
        }

        self.channels.get_mut(&_channel_id).map(|c| {
            c.state = ChannelState::WillReceiveContent(method.consumer_tag.to_string());
            c.queues.iter_mut().map(|(_, ref mut q)| {
              q.consumers.get_mut(&method.consumer_tag).map(|cs| {
                (*cs.callback).start_deliver(_channel_id, &method);
              });
            });
            println!("channel {} state is now {:?}", _channel_id, c.state);
        });
        Ok(())
    }

    pub fn basic_get(&mut self,
                     _channel_id: u16,
                     ticket: ShortUInt,
                     queue: ShortString,
                     no_ack: Boolean)
                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Get(basic::Get {
            ticket: ticket,
            queue: queue,
            no_ack: no_ack,
        }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingBasicGetAnswer;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_basic_get_ok(&mut self,
                                _channel_id: u16,
                                method: basic::GetOk)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingBasicGetAnswer => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Basic.GetOk, ignoring packet");


        Ok(())
    }

    pub fn receive_basic_get_empty(&mut self,
                                   _channel_id: u16,
                                   method: basic::GetEmpty)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingBasicGetAnswer => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Basic.GetEmpty, ignoring packet");


        Ok(())
    }

    pub fn basic_ack(&mut self,
                     _channel_id: u16,
                     delivery_tag: LongLongUInt,
                     multiple: Boolean)
                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Ack(basic::Ack {
            delivery_tag: delivery_tag,
            multiple: multiple,
        }));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn basic_reject(&mut self,
                        _channel_id: u16,
                        delivery_tag: LongLongUInt,
                        requeue: Boolean)
                        -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Reject(basic::Reject {
            delivery_tag: delivery_tag,
            requeue: requeue,
        }));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn basic_recover_async(&mut self, _channel_id: u16, requeue: Boolean) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method =
            Class::Basic(basic::Methods::RecoverAsync(basic::RecoverAsync { requeue: requeue }));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn basic_recover(&mut self, _channel_id: u16, requeue: Boolean) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Recover(basic::Recover { requeue: requeue }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingBasicRecoverOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_basic_recover_ok(&mut self,
                                    _channel_id: u16,
                                    method: basic::RecoverOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingBasicRecoverOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Basic.RecoverOk, ignoring packet");


        Ok(())
    }

    pub fn basic_nack(&mut self,
                      _channel_id: u16,
                      delivery_tag: LongLongUInt,
                      multiple: Boolean,
                      requeue: Boolean)
                      -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Basic(basic::Methods::Nack(basic::Nack {
            delivery_tag: delivery_tag,
            multiple: multiple,
            requeue: requeue,
        }));
        self.send_method_frame(_channel_id, &method)
    }

    pub fn tx_select(&mut self, _channel_id: u16) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Tx(tx::Methods::Select(tx::Select {}));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingTxSelectOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_tx_select_ok(&mut self,
                                _channel_id: u16,
                                method: tx::SelectOk)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingTxSelectOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Tx.SelectOk, ignoring packet");


        Ok(())
    }

    pub fn tx_commit(&mut self, _channel_id: u16) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Tx(tx::Methods::Commit(tx::Commit {}));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingTxCommitOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_tx_commit_ok(&mut self,
                                _channel_id: u16,
                                method: tx::CommitOk)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingTxCommitOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Tx.CommitOk, ignoring packet");


        Ok(())
    }

    pub fn tx_rollback(&mut self, _channel_id: u16) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Tx(tx::Methods::Rollback(tx::Rollback {}));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingTxRollbackOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_tx_rollback_ok(&mut self,
                                  _channel_id: u16,
                                  method: tx::RollbackOk)
                                  -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingTxRollbackOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Tx.RollbackOk, ignoring packet");


        Ok(())
    }



    pub fn confirm_select(&mut self, _channel_id: u16, nowait: Boolean) -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(Error::InvalidChannel);
        }

        if !self.channels
            .get_mut(&_channel_id)
            .map(|c| c.state == ChannelState::Connected)
            .unwrap_or(false) {
            return Err(Error::InvalidState);
        }

        let method = Class::Confirm(confirm::Methods::Select(confirm::Select { nowait: nowait }));

        self.send_method_frame(_channel_id, &method).map(|_| {
            self.channels.get_mut(&_channel_id).map(|c| {
                c.state = ChannelState::AwaitingConfirmSelectOk;
                println!("channel {} state is now {:?}", _channel_id, c.state);
            });
        })
    }

    pub fn receive_confirm_select_ok(&mut self,
                                     _channel_id: u16,
                                     method: confirm::SelectOk)
                                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            println!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(Error::InvalidChannel);
        }

        match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
            ChannelState::Initial | ChannelState::Connected => {}
            ChannelState::Error |
            ChannelState::Closed |
            ChannelState::SendingContent(_) |
            ChannelState::ReceivingContent(_,_) => {
                return Err(Error::InvalidState);
            }
            ChannelState::AwaitingConfirmSelectOk => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
            }
            _ => {
                self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
                return Err(Error::InvalidState);
            }
        }

        println!("unimplemented method Confirm.SelectOk, ignoring packet");


        Ok(())
    }
}
