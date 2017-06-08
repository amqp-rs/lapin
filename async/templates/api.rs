use types::*;
use format::field::*;
use format::frame::*;
use connection::*;
use generated::*;
use error::*;

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ChannelState {
  Initial,
  Connected,
  Closed,
  Error,
  SendingContent(usize),
  ReceivingContent(usize),
  {{#each specs.classes as |class| ~}}
  {{#unless class.is_connection}}
  {{#each class.methods as |method| ~}}{{#if method.synchronous }}Awaiting{{camel class.name}}{{camel method.name}}Ok,
  {{/if}}{{/each ~}}
  {{/unless}}
  {{/each ~}}
}

impl Connection {
  pub fn receive_method(&mut self, channel_id: u16, method: Class) -> Result<(), Error> {
    match method {
      {{#each specs.classes as |class| ~}}
      {{#unless class.is_connection}}
      {{#each class.methods as |method| ~}}
      Class::{{camel class.name}}({{snake class.name}}::Methods::{{camel method.name}}(m)) => {
        self.receive_{{snake class.name}}_{{snake method.name}}(channel_id, m)
      },
      {{/each ~}}
      {{/unless}}
      {{/each ~}}
    }
  }

  {{#each specs.classes as |class| ~}}
  {{#unless class.is_connection}}
  {{#each class.methods as |method| ~}}
  pub fn {{snake class.name}}_{{snake method.name}}(&mut self,
    _channel_id: u16{{#each_argument method.arguments as |argument| ~}},
          {{#if argument_is_value ~}}
            {{snake argument.name}}: {{argument.type}}
          {{else}}
            flags: AMQPFlags
          {{/if ~}}
        {{/each_argument ~}}) -> Result<(), Error> {

      if !self.channels.contains_key(&_channel_id) {
        return Err(Error::InvalidChannel);
      }

      if ! self.channels.get_mut(&_channel_id).map(|c| c.state == ChannelState::Connected).unwrap_or(false) {
        return Err(Error::InvalidState);
      }

      let method = Class::{{camel class.name}}({{snake class.name}}::Methods::{{camel method.name}} (
        {{snake class.name}}::{{camel method.name}} {
          {{#each_argument method.arguments as |argument| ~}}
          {{#if argument_is_value ~}}
          {{snake argument.name}}: {{argument.type}},
          {{else}}
          flags: AMQPFlags,
          {{/if ~}}
          {{/each_argument ~}}
        }
      ));
      {{#if method.synchronous }}
      self.send_method_frame(_channel_id, &method).map(|_| {
        self.channels.get_mut(&_channel_id).map(|c| {
          c.state = ChannelState::Awaiting{{camel class.name}}{{camel method.name}}Ok;
          trace!("channel {} state is now {:?}", _channel_id, c.state);
        });
      }){{else}}self.send_method_frame(_channel_id, &method){{/if}}
  }

  pub fn receive_{{snake class.name}}_{{snake method.name}}(&mut self,
    _channel_id: u16, method: {{snake class.name}}::{{camel method.name}}) -> Result<(), Error> {

      if !self.channels.contains_key(&_channel_id) {
        trace!("key {} not in channels {:?}", _channel_id, self.channels);
        return Err(Error::InvalidChannel);
      }

      match self.channels.get_mut(&_channel_id).map(|c| c.state.clone()).unwrap() {
        ChannelState::Initial | ChannelState::Connected => {},
        ChannelState::Error | ChannelState::Closed | ChannelState::SendingContent(_) | ChannelState::ReceivingContent(_) => {return Err(Error::InvalidState);},
        ChannelState::Awaiting{{camel class.name}}{{camel method.name}} => {
          self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Connected);
        }
        _ => {
          self.channels.get_mut(&_channel_id).map(|c| c.state = ChannelState::Error);
          return Err(Error::InvalidState);
        }
      }

      trace!("unimplemented method {{camel class.name}}.{{camel method.name}}, ignoring packet");


      Ok(())
  }

  {{/each ~}}
  {{/unless}}
{{/each ~}}
}

