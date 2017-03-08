use amq_protocol_types::*;
use format::field::*;
use format::frame::*;
use connection::*;
use generated::*;
use error::*;

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ChannelState {
  Initial,
  Connected,
  Error,
  SendingContent(usize),
  ReceivingContent(usize),
  {{#each specs.classes as |class| ~}}
  {{#each class.methods as |method| ~}}{{#if method.synchronous }}Awaiting{{camel class.name}}{{camel method.name}}Ok,
  {{/if}}{{/each ~}}
  {{/each ~}}
}

impl Connection {

  {{#each specs.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  pub fn {{snake class.name}}_{{snake method.name}}(&mut self,
    _channel_id: u16{{#each method.arguments as |argument| ~}},
    {{snake argument.name}}: {{argument.type}}{{/each ~}}) -> Result<(), Error> {

      if {{class.id}} == 10 && _channel_id != 0 {
        return Err(Error::InvalidChannel);
      }

      if !self.channels.contains_key(&_channel_id) {
        return Err(Error::InvalidChannel);
      }

      if ! self.channels.get_mut(&_channel_id).map(|c| c.state == ChannelState::Connected).unwrap_or(false) {
        return Err(Error::InvalidState);
      }

      let method = Class::{{camel class.name}}({{snake class.name}}::Methods::{{camel method.name}} (
        {{snake class.name}}::{{camel method.name}} {
          {{#each method.arguments as |argument| ~}}
          {{snake argument.name}}: {{snake argument.name}},
          {{/each ~}}
        }
      ));

      {{#if method.synchronous }}
      self.send_method_frame(_channel_id, &method).map(|_| {
        self.channels.get_mut(&_channel_id).map(|c| {
          c.state = ChannelState::Awaiting{{camel class.name}}{{camel method.name}}Ok;
          println!("channel {} state is now {:?}", _channel_id, c.state);
        });
      })
      {{else}}
      self.send_method_frame(_channel_id, &method)
      {{/if}}
  }

  {{/each ~}}
{{/each ~}}
}

