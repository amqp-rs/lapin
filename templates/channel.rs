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
pub(crate) enum Reply {
  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#if method.c2s ~}}
  {{#if method.synchronous ~}}
  Awaiting{{camel class.name}}{{camel method.name}}Ok(WaitHandle<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}>{{#each method.metadata.state as |state| ~}}, {{state.type}}{{/each ~}}),
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
        Err(Error::InvalidMethod(m))
      }
    }
  }

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.metadata.skip ~}}
  {{#if method.c2s ~}}
  {{#unless method.metadata.require_wrapper ~}}{{#unless method.is_reply ~}}pub {{#if method.metadata.internal ~}}(crate) {{/if ~}}{{/unless ~}}fn {{else}}fn do_{{/unless ~}}{{snake class.name false}}_{{snake method.name false}}(&self{{#unless method.ignore_args ~}}{{#each_argument method.arguments as |argument| ~}}{{#if argument_is_value ~}}{{#unless argument.force_default ~}}, {{snake argument.name}}: {{#if (use_str_ref argument.type) ~}}&str{{else}}{{argument.type}}{{/if ~}}{{/unless ~}}{{else}}{{#unless argument.ignore_flags ~}}, options: {{camel class.name}}{{camel method.name}}Options{{/unless ~}}{{/if ~}}{{/each_argument ~}}{{/unless ~}}{{#each method.metadata.extra_args as |arg| ~}}, {{arg.name}}: {{arg.type}}{{/each ~}}) -> Confirmation<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}> {
    {{#if method.metadata.channel_init ~}}
    if !self.status.is_initializing() {
    {{else}}
    {{#if method.metadata.channel_deinit ~}}
    if !self.status.is_closing() {
    {{else}}
    if !self.status.is_connected() {
    {{/if ~}}
    {{/if ~}}
      return Confirmation::new_error(Error::NotConnected);
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
      {{snake argument.name}}: {{snake argument.name}}{{#if (use_str_ref argument.type) ~}}.into(){{/if ~}},
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

    {{#if method.synchronous ~}}
    let (wait, {{#if method.metadata.bypass_wait_handle ~}}_{{/if ~}}wait_handle) = Wait::new();
    {{/if ~}}
    {{#if method.metadata.carry_headers ~}}
    let send_res = self.send_method_frame_with_body(method, payload, properties);
    {{else}}
    let send_res = self.send_method_frame(method, {{#if method.synchronous ~}}Some((Reply::Awaiting{{camel class.name}}{{camel method.name}}Ok({{#if method.metadata.bypass_wait_handle ~}}_{{/if ~}}wait_handle.clone(){{#each method.metadata.state as |state| ~}}, {{state.name}}{{#if state.use_str_ref ~}}.into(){{/if ~}}{{/each ~}}), Box::new({{#if method.metadata.bypass_wait_handle ~}}_{{/if ~}}wait_handle))){{else}}None{{/if ~}});
    {{/if ~}}
    if let Err(err) = send_res {
      return Confirmation::new_error(err);
    }
    {{#if method.metadata.end_hook ~}}
    let end_hook_res = self.on_{{snake class.name false}}_{{snake method.name false}}_sent({{#each method.metadata.end_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}});
    if let Err(err) = end_hook_res {
      return Confirmation::new_error(err);
    }
    {{/if ~}}

    {{#if method.synchronous ~}}
    {{#if (method_has_flag method "nowait") ~}}
    if nowait {
      {{#if method.metadata.nowait_hook ~}}
      if let Err(err) = self.receive_{{snake class.name false}}_{{snake method.name false}}_ok(protocol::{{snake class.name}}::{{camel method.name}}Ok { {{#each method.metadata.nowait_hook.fields as |field| ~}}{{field}}, {{/each ~}}..Default::default() }) {
        return Confirmation::new_error(err);
      }
      {{/if ~}}
    }
    {{/if ~}}
    Confirmation::new(wait)
    {{else}}
    Confirmation::new(send_res.unwrap())
    {{/if ~}}
  }
  {{/if ~}}

  {{#if method.s2c ~}}
  {{#if method.is_reply ~}}
  fn receive_{{snake class.name false}}_{{snake method.name false}}(&self, {{#if method.arguments ~}}method{{else}}_{{/if ~}}: protocol::{{snake class.name}}::{{camel method.name}}) -> Result<(), Error> {
    {{#if method.metadata.channel_init ~}}
    if !self.status.is_initializing() {
    {{else}}
    {{#if method.metadata.channel_deinit ~}}
    if !self.status.is_closing() {
    {{else}}
    if !self.status.is_connected() {
    {{/if ~}}
    {{/if ~}}
      return Err(Error::NotConnected);
    }

    match self.connection.next_expected_reply(self.id) {
      Some(Reply::Awaiting{{camel class.name}}{{camel method.name}}(wait_handle{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}})) => {
        {{#unless method.metadata.confirmation.type ~}}
        wait_handle.finish(Default::default());
        {{/unless ~}}
        {{#if method.arguments ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received(method{{#if method.metadata.confirmation.type ~}}, wait_handle{{/if ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}})
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
        Err(Error::UnexpectedReply)
      },
    }
  }
  {{else}}
  fn receive_{{snake class.name false}}_{{snake method.name false}}(&self, method: protocol::{{snake class.name}}::{{camel method.name}}) -> Result<(), Error> {
    if !self.status.is_connected() {
      return Err(Error::NotConnected);
    }
    self.on_{{snake class.name false}}_{{snake method.name false}}_received(method)
  }
  {{/if ~}}
  {{/if ~}}
  {{/unless ~}}
  {{/each ~}}
  {{/each ~}}
}
