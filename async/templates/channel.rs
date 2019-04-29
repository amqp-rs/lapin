pub mod options {
  use super::*;

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#each_argument method.arguments as |argument| ~}}
  {{#unless argument_is_value ~}}
  #[derive(Clone, Debug, Default, PartialEq)]
  pub struct {{camel class.name}}{{camel method.name}}Options {
    {{#each_flag argument as |flag| ~}}
    pub {{snake flag.name}}: Boolean,
    {{/each_flag ~}}
  }

  {{/unless ~}}
  {{/each_argument ~}}
  {{/each ~}}
  {{/each ~}}
}

use options::*;

impl ChannelHandle {
  {{#each protocol.classes as |class| ~}}
  {{#unless class.metadata.skip ~}}
  {{#each class.methods as |method| ~}}
  pub fn {{snake class.name false}}_{{snake method.name false}}(&mut self{{#each_argument method.arguments as |argument| ~}}{{#if argument_is_value ~}}{{#unless argument.force_default ~}}, {{snake argument.name}}: {{#if (use_str_ref argument.type) ~}}&str{{else}}{{argument.type}}{{/if ~}}{{/unless ~}}{{else}}, options: {{camel class.name}}{{camel method.name}}Options{{/if ~}}{{/each_argument ~}}{{#each method.metadata.extra_args as |arg| ~}}, {{arg.name}}: {{arg.type}}{{/each ~}}) -> Result<Option<RequestId>, Error> {
    {{#if method.metadata.channel_init ~}}
    if !self.is_initializing() {
    {{else}}
    if !self.is_connected() {
    {{/if ~}}
      return Err(ErrorKind::NotConnected.into());
    }

    {{#each_argument method.arguments as |argument| ~}}
    {{#unless argument_is_value ~}}
    let {{camel class.name}}{{camel method.name}}Options {
      {{#each_flag argument as |flag| ~}}
      {{snake flag.name}},
      {{/each_flag ~}}
    } = options;
    {{/unless ~}}
    {{/each_argument ~}}

    let method = AMQPClass::{{camel class.name}}(protocol::{{snake class.name}}::AMQPMethod::{{camel method.name}} (protocol::{{snake class.name}}::{{camel method.name}} {
      {{#each_argument method.arguments as |argument| ~}}
      {{#if argument_is_value ~}}
      {{#unless argument.force_default ~}}
      {{snake argument.name}}: {{snake argument.name}}{{#if (use_str_ref argument.type) ~}}.to_string(){{/if ~}},
      {{/unless ~}}
      {{else}}
      {{#each_flag argument as |flag| ~}}
      {{snake flag.name}},
      {{/each_flag ~}}
      {{/if ~}}
      {{/each_argument ~}}
    }));

    self.send_method_frame(method);

    {{#if method.metadata.end_hook ~}}
    self.on_{{snake class.name false}}_{{snake method.name false}}({{#each method.metadata.end_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}});
    {{/if ~}}

    Ok({{#unless method.synchronous ~}}None{{else}}
      {{#if (method_has_flag method "nowait") ~}}
      if nowait {
        None
      } else
      {{/if ~}}
      {
        let request_id = self.next_request_id();
        self.await_answer(Answer::Awaiting{{camel class.name}}{{camel method.name}}Ok(request_id{{#each method.metadata.state as |state| ~}}, {{state}}{{#if (use_str_ref (argument_type method state)) ~}}.to_string(){{/if ~}}{{/each ~}}));
        Some(request_id)
      }
    {{/unless ~}})
  }

  {{/each ~}}
  {{/unless ~}}
  {{/each ~}}
}
