use amq_protocol::protocol::{AMQPClass, AMQPError, AMQPSoftError, access, basic, channel, confirm, exchange, queue};
use log::{error, info, trace};

use crate::channel::options::*;
use crate::connection::*;
use crate::consumer::*;
use crate::queue::*;
use crate::message::*;
use crate::error::*;

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ChannelState {
    Initial,
    Connected,
    Closed,
    Error,
    SendingContent(usize),
    WillReceiveContent(String,Option<String>),
    ReceivingContent(String,Option<String>,usize),
}

pub type RequestId = u64;

#[derive(Debug)]
pub enum Answer {
    AwaitingChannelOpenOk(RequestId),
    AwaitingChannelFlowOk(RequestId),
    AwaitingChannelCloseOk(RequestId),

    AwaitingAccessRequestOk(RequestId),

    AwaitingExchangeDeclareOk(RequestId),
    AwaitingExchangeDeleteOk(RequestId),
    AwaitingExchangeBindOk(RequestId),
    AwaitingExchangeUnbindOk(RequestId),

    AwaitingQueueDeclareOk(RequestId),
    AwaitingQueueBindOk(RequestId),
    AwaitingQueuePurgeOk(RequestId),
    AwaitingQueueDeleteOk(RequestId, String),
    AwaitingQueueUnbindOk(RequestId),

    AwaitingBasicQosOk(RequestId, u16,bool),
    AwaitingBasicConsumeOk(RequestId, String, String, bool, bool, bool, bool, Box<dyn ConsumerSubscriber>),
    AwaitingBasicCancelOk(RequestId),
    AwaitingBasicGetOk(RequestId, String),
    AwaitingBasicRecoverOk(RequestId),

    AwaitingTxSelectOk(RequestId),
    AwaitingTxCommitOk(RequestId),
    AwaitingTxRollbackOk(RequestId),

    // RabbitMQ confirm extension
    AwaitingConfirmSelectOk(RequestId),
    AwaitingPublishConfirm(RequestId),
}

macro_rules! try_unacked (
  ($channel: ident, $method: expr, $ack: expr) => ({
    if let Err(e) = $ack {
      $channel.handle().channel_close(AMQPSoftError::PRECONDITIONFAILED.get_id(), "precondition failed", 60, $method)?;
      return Err(e);
    }
  });
);

#[allow(clippy::all)]
impl Connection {
    pub fn receive_method(&mut self, channel_id: u16, method: AMQPClass) -> Result<(), Error> {
        match method {

            AMQPClass::Channel(channel::AMQPMethod::OpenOk(m)) => {
                self.receive_channel_open_ok(channel_id, m)
            }
            AMQPClass::Channel(channel::AMQPMethod::Flow(m)) => self.receive_channel_flow(channel_id, m),
            AMQPClass::Channel(channel::AMQPMethod::FlowOk(m)) => {
                self.receive_channel_flow_ok(channel_id, m)
            }
            AMQPClass::Channel(channel::AMQPMethod::Close(m)) => self.receive_channel_close(channel_id, m),
            AMQPClass::Channel(channel::AMQPMethod::CloseOk(m)) => {
                self.receive_channel_close_ok(channel_id, m)
            }

            AMQPClass::Access(access::AMQPMethod::RequestOk(m)) => {
                self.receive_access_request_ok(channel_id, m)
            }

            AMQPClass::Exchange(exchange::AMQPMethod::DeclareOk(m)) => {
                self.receive_exchange_declare_ok(channel_id, m)
            }
            AMQPClass::Exchange(exchange::AMQPMethod::DeleteOk(m)) => {
                self.receive_exchange_delete_ok(channel_id, m)
            }
            AMQPClass::Exchange(exchange::AMQPMethod::BindOk(m)) => {
                self.receive_exchange_bind_ok(channel_id, m)
            }
            AMQPClass::Exchange(exchange::AMQPMethod::UnbindOk(m)) => {
                self.receive_exchange_unbind_ok(channel_id, m)
            }

            AMQPClass::Queue(queue::AMQPMethod::DeclareOk(m)) => {
                self.receive_queue_declare_ok(channel_id, m)
            }
            AMQPClass::Queue(queue::AMQPMethod::BindOk(m)) => self.receive_queue_bind_ok(channel_id, m),
            AMQPClass::Queue(queue::AMQPMethod::PurgeOk(m)) => self.receive_queue_purge_ok(channel_id, m),
            AMQPClass::Queue(queue::AMQPMethod::DeleteOk(m)) => {
                self.receive_queue_delete_ok(channel_id, m)
            }
            AMQPClass::Queue(queue::AMQPMethod::UnbindOk(m)) => {
                self.receive_queue_unbind_ok(channel_id, m)
            }

            AMQPClass::Basic(basic::AMQPMethod::QosOk(m)) => self.receive_basic_qos_ok(channel_id, m),
            AMQPClass::Basic(basic::AMQPMethod::ConsumeOk(m)) => {
                self.receive_basic_consume_ok(channel_id, m)
            }
            AMQPClass::Basic(basic::AMQPMethod::Cancel(m)) => {
                self.receive_basic_cancel(channel_id, m)
            }
            AMQPClass::Basic(basic::AMQPMethod::CancelOk(m)) => {
                self.receive_basic_cancel_ok(channel_id, m)
            }
            AMQPClass::Basic(basic::AMQPMethod::Deliver(m)) => self.receive_basic_deliver(channel_id, m),
            AMQPClass::Basic(basic::AMQPMethod::GetOk(m)) => self.receive_basic_get_ok(channel_id, m),
            AMQPClass::Basic(basic::AMQPMethod::GetEmpty(m)) => {
                self.receive_basic_get_empty(channel_id, m)
            }
            AMQPClass::Basic(basic::AMQPMethod::RecoverOk(m)) => {
                self.receive_basic_recover_ok(channel_id, m)
            }

            /*
            AMQPClass::Tx(tx::Methods::SelectOk(m)) => self.receive_tx_select_ok(channel_id, m),
            AMQPClass::Tx(tx::Methods::CommitOk(m)) => self.receive_tx_commit_ok(channel_id, m),
            AMQPClass::Tx(tx::Methods::RollbackOk(m)) => self.receive_tx_rollback_ok(channel_id, m),
            */

            AMQPClass::Confirm(confirm::AMQPMethod::SelectOk(m)) => {
                self.receive_confirm_select_ok(channel_id, m)
            }
            AMQPClass::Basic(basic::AMQPMethod::Ack(m)) => {
                self.receive_basic_ack(channel_id, m)
            }
            AMQPClass::Basic(basic::AMQPMethod::Nack(m)) => {
                self.receive_basic_nack(channel_id, m)
            }

            m => {
                error!("the client should not receive this method: {:?}", m);
                return Err(ErrorKind::InvalidMethod(m).into());
            }
        }
    }

    pub fn receive_channel_open_ok(&mut self,
                                   _channel_id: u16,
                                   _: channel::OpenOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if let Err(err) = self.check_state(_channel_id, ChannelState::Initial) {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(err);
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingChannelOpenOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }

        self.set_channel_state(_channel_id, ChannelState::Connected);
        Ok(())
    }

    pub fn receive_channel_flow(&mut self,
                                _channel_id: u16,
                                method: channel::Flow)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        if let Some(channel) = self.channels.get_mut(&_channel_id) {
          let active = method.active;
          channel.send_flow = active;
          channel.handle().channel_flow_ok(ChannelFlowOkOptions { active })?;
        }
        Ok(())
    }

    pub fn receive_channel_flow_ok(&mut self,
                                   _channel_id: u16,
                                   method: channel::FlowOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingChannelFlowOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            self.channels.get_mut(&_channel_id).map(|c| c.receive_flow = method.active);
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }

        Ok(())
    }

    pub fn receive_channel_close(&mut self,
                                 _channel_id: u16,
                                 close: channel::Close)
                                 -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        if let Some(error) = AMQPError::from_id(close.reply_code) {
            error!("Channel {} closed by {}:{} => {:?} => {}", _channel_id, close.class_id, close.method_id, error, close.reply_text);
        } else {
            info!("Channel {} closed: {:?}", _channel_id, close);
        }

        self.get_next_answer(_channel_id);
        if let Some(channel) = self.channels.get_mut(&_channel_id) {
          channel.handle().channel_close_ok()?;
        }
        self.set_channel_state(_channel_id, ChannelState::Closed);
        Ok(())
    }

    pub fn receive_channel_close_ok(&mut self,
                                    _channel_id: u16,
                                    _: channel::CloseOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if ! self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingChannelCloseOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            self.set_channel_state(_channel_id, ChannelState::Closed);
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }

        Ok(())
    }

    pub fn receive_access_request_ok(&mut self,
                                     _channel_id: u16,
                                     _: access::RequestOk)
                                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingAccessRequestOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_exchange_declare_ok(&mut self,
                                       _channel_id: u16,
                                       _: exchange::DeclareOk)
                                       -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingExchangeDeclareOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_exchange_delete_ok(&mut self,
                                      _channel_id: u16,
                                      _: exchange::DeleteOk)
                                      -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingExchangeDeleteOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_exchange_bind_ok(&mut self,
                                    _channel_id: u16,
                                    _: exchange::BindOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingExchangeBindOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_exchange_unbind_ok(&mut self,
                                      _channel_id: u16,
                                      _: exchange::UnbindOk)
                                      -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingExchangeUnbindOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_queue_declare_ok(&mut self,
                                    _channel_id: u16,
                                    method: queue::DeclareOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingQueueDeclareOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            self.generated_names.insert(request_id, method.queue.clone());
            self.channels.get_mut(&_channel_id).map(|c| {
              let q = Queue::new(method.queue.clone(), method.message_count, method.consumer_count);
              c.register_queue(q);
            });
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_queue_bind_ok(&mut self,
                                 _channel_id: u16,
                                 _: queue::BindOk)
                                 -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingQueueBindOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_queue_purge_ok(&mut self,
                                  _channel_id: u16,
                                  _: queue::PurgeOk)
                                  -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }


        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingQueuePurgeOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_queue_delete_ok(&mut self,
                                   _channel_id: u16,
                                   _: queue::DeleteOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingQueueDeleteOk(request_id, key)) => {
            self.finished_reqs.insert(request_id, true);
            self.channels.get_mut(&_channel_id).map(|c| c.deregister_queue(&key));
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_queue_unbind_ok(&mut self,
                                   _channel_id: u16,
                                   _: queue::UnbindOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingQueueUnbindOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_basic_qos_ok(&mut self,
                                _channel_id: u16,
                                _: basic::QosOk)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingBasicQosOk(request_id, prefetch_count, global)) => {
            self.finished_reqs.insert(request_id, true);
            if global {
              self.prefetch_count = prefetch_count;
            } else {
              self.channels.get_mut(&_channel_id).map(|c| {
                c.prefetch_count = prefetch_count;
              });
            }
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_basic_consume_ok(&mut self,
                                    _channel_id: u16,
                                    method: basic::ConsumeOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingBasicConsumeOk(request_id, queue, _, no_local, no_ack, exclusive, nowait, subscriber)) => {
            self.finished_reqs.insert(request_id, true);
            self.generated_names.insert(request_id, method.consumer_tag.clone());
            self.channels.get_mut(&_channel_id).map(|c| {
              let consumer = Consumer::new(method.consumer_tag.clone(), no_local, no_ack, exclusive, nowait, subscriber);
              c.register_consumer(&queue, &method.consumer_tag, consumer);
            });
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_basic_cancel(&mut self,
                                _channel_id: u16,
                                method: basic::Cancel)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        if let Some(channel) = self.channels.get_mut(&_channel_id) {
            channel.deregister_consumer(&method.consumer_tag);
            if !method.nowait {
                channel.handle().basic_cancel_ok(&method.consumer_tag)?;
            }
        }
        Ok(())
    }

    pub fn receive_basic_cancel_ok(&mut self,
                                   _channel_id: u16,
                                   method: basic::CancelOk)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingBasicCancelOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            if let Some(channel) = self.channels.get_mut(&_channel_id) {
              channel.deregister_consumer(&method.consumer_tag);
            }
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_basic_deliver(&mut self,
                                 _channel_id: u16,
                                 method: basic::Deliver)
                                 -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        if let Some(c) = self.channels.get_mut(&_channel_id) {
          let message = Delivery::new(
            method.delivery_tag,
            method.exchange.to_string(),
            method.routing_key.to_string(),
            method.redelivered
          );
          c.start_consumer_delivery(&method.consumer_tag, message);
        }
        Ok(())
    }

    pub fn receive_basic_get_ok(&mut self,
                                _channel_id: u16,
                                method: basic::GetOk)
                                -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingBasicGetOk(request_id, queue_name)) => {
            self.finished_get_reqs.insert(request_id, true);
            self.set_channel_state(_channel_id, ChannelState::WillReceiveContent(queue_name.to_string(), None));

            if let Some(c) = self.channels.get_mut(&_channel_id) {
              let message = BasicGetMessage::new(
                  method.delivery_tag,
                  method.exchange.to_string(),
                  method.routing_key.to_string(),
                  method.redelivered,
                  method.message_count
              );
              c.start_basic_get_delivery(&queue_name, message);
            }

            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_basic_get_empty(&mut self,
                                   _channel_id: u16,
                                   _: basic::GetEmpty)
                                   -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingBasicGetOk(request_id, _)) => {
            self.finished_get_reqs.insert(request_id, false);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    fn drop_prefetched_messages(&mut self, channel_id: u16) {
        if let Some(channel) = self.channels.get_mut(&channel_id) {
          channel.handle().drop_prefetched_messages();
        }
    }

    pub fn receive_basic_recover_ok(&mut self,
                                    _channel_id: u16,
                                    _: basic::RecoverOk)
                                    -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingBasicRecoverOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            self.drop_prefetched_messages(_channel_id);
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_confirm_select_ok(&mut self,
                                     _channel_id: u16,
                                     _: confirm::SelectOk)
                                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            trace!("key {} not in channels {:?}", _channel_id, self.channels);
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingConfirmSelectOk(request_id)) => {
            self.finished_reqs.insert(request_id, true);
            self.channels.get_mut(&_channel_id).map(|c| {
              c.set_confirm();
            });
            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_basic_ack(&mut self,
                     _channel_id: u16,
                     method: basic::Ack)
                     -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingPublishConfirm(_)) => {
            if let Some(c) = self.channels.get_mut(&_channel_id) {
              if c.confirm() {
                if method.multiple {
                  if method.delivery_tag > 0 {
                    for tag in c.all_unacked_before(method.delivery_tag) {
                      try_unacked!(c, 80, c.ack(tag));
                    }
                  } else {
                    for tag in c.drain_all_unacked() {
                      c.acked.insert(tag);
                    }
                  }
                } else {
                  try_unacked!(c, 80, c.ack(method.delivery_tag));
                }
              }
            };

            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

    pub fn receive_basic_nack(&mut self,
                      _channel_id: u16,
                      method: basic::Nack)
                      -> Result<(), Error> {

        if !self.channels.contains_key(&_channel_id) {
            return Err(ErrorKind::InvalidChannel(_channel_id).into());
        }

        if !self.is_connected(_channel_id) {
            return Err(ErrorKind::NotConnected.into());
        }

        match self.get_next_answer(_channel_id) {
          Some(Answer::AwaitingPublishConfirm(_)) => {
            if let Some(c) = self.channels.get_mut(&_channel_id) {
              if c.confirm() {
                if method.multiple {
                  if method.delivery_tag > 0 {
                    for tag in c.all_unacked_before(method.delivery_tag) {
                      try_unacked!(c, 120, c.nack(tag));
                    }
                  } else {
                    for tag in c.drain_all_unacked() {
                      c.nacked.insert(tag);
                    }
                  }
                } else {
                  try_unacked!(c, 120, c.nack(method.delivery_tag));
                }
              }
            };

            Ok(())
          },
          _ => {
            self.set_channel_state(_channel_id, ChannelState::Error);
            return Err(ErrorKind::UnexpectedAnswer.into());
          }
        }
    }

}
