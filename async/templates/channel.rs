pub mod options {
  use super::*;

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.ignore_args ~}}
  {{#each_argument method.arguments as |argument| ~}}
  {{#unless argument_is_value ~}}
  {{#unless argument.ignore_flags ~}}
  #[derive(Clone, Debug, Default, PartialEq)]
  pub struct {{camel class.name}}{{camel method.name}}Options {
    {{#each argument.flags as |flag| ~}}
    pub {{snake flag.name}}: Boolean,
    {{/each ~}}
  }

  {{/unless ~}}
  {{/unless ~}}
  {{/each_argument ~}}
  {{/unless ~}}
  {{/each ~}}
  {{/each ~}}
}

use options::*;

#[derive(Debug)]
pub enum Reply {
  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#if method.c2s ~}}
  {{#if method.synchronous ~}}
  Awaiting{{camel class.name}}{{camel method.name}}Ok(RequestId{{#each method.metadata.state as |state| ~}}, {{state.type}}{{/each ~}}),
  {{/if ~}}
  {{/if ~}}
  {{/each ~}}
  {{/each ~}}
}

impl Channel {
  pub(crate) fn receive_method(&self, method: AMQPClass) -> Result<(), Error> {
    match method {
      {{#each protocol.classes as |class| ~}}
      {{#each class.methods as |method| ~}}
      {{#unless method.metadata.skip ~}}
      {{#if method.s2c ~}}
      AMQPClass::{{camel class.name}}(protocol::{{snake class.name}}::AMQPMethod::{{camel method.name}}(m)) => self.receive_{{snake class.name false}}_{{snake method.name false}}(m),
      {{/if ~}}
      {{/unless ~}}
      {{/each ~}}
      {{/each ~}}
      m => {
        error!("the client should not receive this method: {:?}", m);
        Err(ErrorKind::InvalidMethod(m).into())
      }
    }
  }

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.metadata.skip ~}}
  {{#if method.c2s ~}}
  pub fn {{snake class.name false}}_{{snake method.name false}}(&self{{#unless method.ignore_args ~}}{{#each_argument method.arguments as |argument| ~}}{{#if argument_is_value ~}}{{#unless argument.force_default ~}}, {{snake argument.name}}: {{#if (use_str_ref argument.type) ~}}&str{{else}}{{argument.type}}{{/if ~}}{{/unless ~}}{{else}}{{#unless argument.ignore_flags ~}}, options: {{camel class.name}}{{camel method.name}}Options{{/unless ~}}{{/if ~}}{{/each_argument ~}}{{/unless ~}}{{#each method.metadata.extra_args as |arg| ~}}, {{arg.name}}: {{arg.type}}{{/each ~}}) -> Result<Option<{{#if method.metadata.end_hook.return_type ~}}{{method.metadata.end_hook.return_type}}{{else}}RequestId{{/if ~}}>, Error> {
    {{#if method.metadata.channel_init ~}}
    if !self.status.is_initializing() {
    {{else}}
    if !self.status.is_connected() {
    {{/if ~}}
      return Err(ErrorKind::NotConnected.into());
    }

    {{#unless method.ignore_args ~}}
    {{#each_argument method.arguments as |argument| ~}}
    {{#unless argument_is_value ~}}
    {{#unless argument.ignore_flags ~}}
    let {{camel class.name}}{{camel method.name}}Options {
      {{#each argument.flags as |flag| ~}}
      {{snake flag.name}}{{#if flag.force_default ~}}: _{{/if ~}},
      {{/each ~}}
    } = options;
    {{/unless ~}}
    {{/unless ~}}
    {{/each_argument ~}}
    {{/unless ~}}

    let method = AMQPClass::{{camel class.name}}(protocol::{{snake class.name}}::AMQPMethod::{{camel method.name}} (protocol::{{snake class.name}}::{{camel method.name}} {
      {{#each_argument method.arguments as |argument| ~}}
      {{#if argument_is_value ~}}
      {{#unless argument.force_default ~}}
      {{snake argument.name}}: {{snake argument.name}}{{#if (use_str_ref argument.type) ~}}.to_string(){{/if ~}},
      {{/unless ~}}
      {{else}}
      {{#unless argument.ignore_flags ~}}
      {{#each argument.flags as |flag| ~}}
      {{#unless flag.force_default ~}}
      {{snake flag.name}},
      {{/unless ~}}
      {{/each ~}}
      {{/unless ~}}
      {{/if ~}}
      {{/each_argument ~}}
    }));

    self.send_method_frame(method);

    {{#if method.metadata.end_hook ~}}
    {{#if method.metadata.end_hook.return_type ~}}let end_hook_ret = {{/if ~}}self.on_{{snake class.name false}}_{{snake method.name false}}_sent({{#each method.metadata.end_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}})?;
    {{/if ~}}

    Ok(
      {{#if method.synchronous ~}}
      {{#if (method_has_flag method "nowait") ~}}
      if nowait {
        {{#if method.metadata.nowait_hook ~}}
        #[allow(clippy::needless_update)]
        self.on_{{snake class.name false}}_{{snake method.name false}}_ok_received({{#unless method.metadata.nowait_hook.no_args ~}}protocol::{{snake class.name}}::{{camel method.name}}Ok { {{#each method.metadata.nowait_hook.fields as |field| ~}}{{field}}, {{/each ~}}..Default::default() }{{#each method.metadata.nowait_hook.extra_args as |arg| ~}}, {{arg}}{{/each ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{#if state.use_str_ref ~}}.to_string(){{/if ~}}{{/each ~}}{{/unless ~}})?;
        {{/if ~}}
        None
      } else {{/if ~}}{
        let request_id = self.request_id.next();
        self.replies.register_pending(self.id, Reply::Awaiting{{camel class.name}}{{camel method.name}}Ok(request_id{{#each method.metadata.state as |state| ~}}, {{state.name}}{{#if state.use_str_ref ~}}.to_string(){{/if ~}}{{/each ~}}));
        Some(request_id)
      }
      {{else}}
      {{#if method.metadata.end_hook.return_type ~}}
      end_hook_ret
      {{else}}
      None
      {{/if}}
      {{/if ~}}
    )
  }
  {{/if ~}}

  {{#if method.s2c ~}}
  {{#if method.is_reply ~}}
  fn receive_{{snake class.name false}}_{{snake method.name false}}(&self, {{#if method.arguments ~}}method{{else}}_{{/if ~}}: protocol::{{snake class.name}}::{{camel method.name}}) -> Result<(), Error> {
    {{#if method.metadata.channel_init ~}}
    if !self.status.is_initializing() {
    {{else}}
    if !self.status.is_connected() {
    {{/if ~}}
      return Err(ErrorKind::NotConnected.into());
    }

    match self.replies.next() {
      Some(Reply::Awaiting{{camel class.name}}{{camel method.name}}(request_id{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}})) => {
        self.requests.finish(request_id, true);
        {{#if method.arguments ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received(method{{#if method.metadata.uses_request_id ~}}, request_id{{/if ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}})
        {{else}}
        {{#if method.metadata.received_hook ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received({{#each method.metadata.received_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}})
        {{else}}
        Ok(())
        {{/if ~}}
        {{/if ~}}
      },
      _ => {
        self.set_error()?;
        Err(ErrorKind::UnexpectedReply.into())
      },
    }
  }
  {{else}}
  fn receive_{{snake class.name false}}_{{snake method.name false}}(&self, method: protocol::{{snake class.name}}::{{camel method.name}}) -> Result<(), Error> {
    if !self.status.is_connected() {
      return Err(ErrorKind::NotConnected.into());
    }
    self.on_{{snake class.name false}}_{{snake method.name false}}_received(method)
  }
  {{/if ~}}
  {{/if ~}}
  {{/unless ~}}
  {{/each ~}}
  {{/each ~}}
}
