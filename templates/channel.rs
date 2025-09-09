pub mod options {
  use super::*;

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.ignore_args ~}}
  {{#each_argument method.arguments as |argument| ~}}
  {{#unless @argument_is_value ~}}
  {{#unless argument.ignore_flags ~}}
  #[derive(Copy, Clone, Debug, Default, PartialEq)]
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
#[allow(clippy::enum_variant_names)]
pub(crate) enum Reply {
  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#if method.c2s ~}}
  {{#if method.synchronous ~}}
  {{camel class.name}}{{camel method.name}}Ok(PromiseResolver<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}>{{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{state.type}}{{/each ~}}{{/if ~}}),
  {{/if ~}}
  {{/if ~}}
  {{/each ~}}
  {{/each ~}}
}

impl Channel {
  pub(crate) fn receive_method(&self, method: AMQPClass) -> Result<()> {
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
        error!(method=?m, "The client should not receive this method");
        self.handle_invalid_contents(format!("unexpected method received on channel {}", self.id), m.get_amqp_class_id(), m.get_amqp_method_id())
      }
    }
  }

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.metadata.skip ~}}
  {{#if method.c2s ~}}
{{include_more class.name method.name}}{{#unless method.metadata.require_wrapper ~}}{{#if method.is_reply ~}}{{#if method.metadata.internal ~}}pub(crate) {{/if ~}}{{else}}pub {{#if method.metadata.internal ~}}(crate) {{/if ~}}{{/if ~}}async fn {{else}}async fn do_{{/unless ~}}{{snake class.name false}}_{{snake method.name false}}(&self{{#unless method.ignore_args ~}}{{#each_argument method.arguments as |argument| ~}}{{#if @argument_is_value ~}}{{#unless argument.force_default ~}}, {{snake argument.name}}: {{#if (use_str_ref argument.type) ~}}&str{{else}}{{argument.type}}{{/if ~}}{{/unless ~}}{{else}}{{#unless argument.ignore_flags ~}}, options: {{camel class.name}}{{camel method.name}}Options{{/unless ~}}{{/if ~}}{{/each_argument ~}}{{/unless ~}}{{#if method.metadata.extra_args ~}}{{#each method.metadata.extra_args as |arg| ~}}, {{arg.name}}: {{arg.type}}{{/each ~}}{{/if ~}}) -> Result<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}> {
    {{#unless class.metadata.channel0_only ~}}
    {{#if method.metadata.channel_init ~}}
    if !self.status.initializing() {
    {{else}}
    {{#if method.metadata.channel_deinit ~}}
    if !self.status.closing() {
    {{else}}
    {{#if method.metadata.channel_recovery ~}}
    if !self.status.connected_or_recovering() {
    {{else}}
    if !self.status.connected() {
    {{/if ~}}
    {{/if ~}}
    {{/if ~}}
      return Err(self.status.state_error("{{class.name}}.{{method.name}}"));
    }

    {{/unless ~}}
    {{#if method.metadata.start_hook ~}}
    {{#if method.metadata.start_hook.returns ~}}let start_hook_res = {{/if ~}}self.before_{{snake class.name false}}_{{snake method.name false}}({{#if method.metadata.start_hook.params ~}}{{#each method.metadata.start_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}}{{/if ~}});
    {{/if ~}}

    {{#if method.metadata.init_clones ~}}
    {{#each method.metadata.init_clones as |init_clone| ~}}
    let {{init_clone.to}} = {{init_clone.from}}.clone();
    {{/each ~}}
    {{/if ~}}
    {{#unless method.ignore_args ~}}
    {{#each_argument method.arguments as |argument| ~}}
    {{#unless @argument_is_value ~}}
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
      {{#if @argument_is_value ~}}
      {{#unless argument.force_default ~}}
      {{snake argument.name}}{{#if (use_str_ref argument.type) ~}}: {{snake argument.name}}.into(){{/if ~}},
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

    let (promise, resolver) = Promise::new();
    if level_enabled!(Level::TRACE) {
      promise.set_marker("{{class.name}}.{{method.name}}".into());
    }
    {{#if method.metadata.carry_headers ~}}
    let send_res = self.send_method_frame_with_body(method, payload, properties, start_hook_res, resolver);
    promise.await.and(send_res)
    {{else}}
    {{#if method.metadata.resolver_hook ~}}{{method.metadata.resolver_hook}}{{/if ~}}
    {{#if method.metadata.send_hook ~}}
    self.before_{{snake class.name false}}_{{snake method.name false}}({{#if method.metadata.send_hook.params ~}}{{#each method.metadata.send_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}}{{/if ~}});
    {{/if ~}}
    self.send_method_frame(method, Box::new(resolver.clone()), {{#if method.synchronous ~}}Some(ExpectedReply(Reply::{{camel class.name}}{{camel method.name}}Ok(resolver.clone(){{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{#if state.provider}}{{state.provider}}{{else}}{{state.name}}{{#if state.use_str_ref ~}}.into(){{/if ~}}{{/if ~}}{{/each ~}}{{/if ~}}), Box::new(resolver))), None{{else}}None, Some(resolver){{/if ~}});
    {{#if method.metadata.end_hook ~}}
    self.on_{{snake class.name false}}_{{snake method.name false}}_sent({{#if method.metadata.end_hook.params ~}}{{#each method.metadata.end_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}}{{/if ~}});
    {{/if ~}}

    {{#if method.synchronous ~}}
    {{#if method.metadata.nowait_hook ~}}
    if nowait {
      self.receive_{{snake class.name false}}_{{snake method.name false}}_ok(protocol::{{snake class.name}}::{{camel method.name}}Ok { {{#if method.metadata.nowait_hook.fields ~}}{{#each method.metadata.nowait_hook.fields as |field| ~}}{{field}}, {{/each ~}}{{/if ~}}{{#if method.metadata.nowait_hook.nonexhaustive_args ~}}..Default::default(){{/if ~}} })?;
    }
    {{/if ~}}
    {{/if ~}}
    promise.await
    {{/if ~}}
  }
  {{/if ~}}

  {{#if method.s2c ~}}
  {{#if method.is_reply ~}}
  fn receive_{{snake class.name false}}_{{snake method.name false}}(&self, method: protocol::{{snake class.name}}::{{camel method.name}}) -> Result<()> {
    {{#if class.metadata.channel0_only ~}}
    self.assert_channel0(
      method.get_amqp_class_id(),
      method.get_amqp_method_id(),
    )?;
    {{/if ~}}
    {{#if method.metadata.channel_init ~}}
    if !self.status.initializing() {
    {{else}}
    if !self.status.can_receive_messages() {
    {{/if ~}}
      return Err(self.status.state_error("{{class.name}}.{{method.name}}"));
    }

    match {{#if method.metadata.expected_reply_getter ~}}{{method.metadata.expected_reply_getter}}{{else}}self.frames.find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::{{camel class.name}}{{camel method.name}}(..))){{/if ~}} {
      Some(Reply::{{camel class.name}}{{camel method.name}}(resolver{{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}}{{/if ~}})) => {
        {{#unless method.metadata.confirmation.type ~}}let res ={{/unless ~}}
        {{#if method.arguments ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received(method{{#if method.metadata.confirmation.type ~}}, resolver{{/if ~}}{{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}}{{/if ~}})
        {{else}}
        {{#if method.metadata.received_hook ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received({{#if method.metadata.received_hook.params ~}}{{#each method.metadata.received_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}}{{/if ~}})
        {{else}}
        Ok(())
        {{/if ~}}
        {{/if ~}}
        {{#unless method.metadata.confirmation.type ~}};
        resolver.complete(res.clone());
        res
        {{/unless ~}}
      },
      unexpected => {
        self.handle_invalid_contents(format!("unexpected {{class.name}} {{method.name}} received on channel {}, was awaiting for {:?}", self.id, unexpected), method.get_amqp_class_id(), method.get_amqp_method_id())
      },
    }
  }
  {{else}}
  fn receive_{{snake class.name false}}_{{snake method.name false}}(&self, method: protocol::{{snake class.name}}::{{camel method.name}}) -> Result<()> {
    {{#if class.metadata.channel0_only ~}}
    self.assert_channel0(
      method.get_amqp_class_id(),
      method.get_amqp_method_id(),
    )?;
    {{/if ~}}
    if !self.status.can_receive_messages() {
      return Err(self.status.state_error("{{class.name}}.{{method.name}}"));
    }
    self.on_{{snake class.name false}}_{{snake method.name false}}_received(method)
  }
  {{/if ~}}
  {{/if ~}}
  {{/unless ~}}
  {{/each ~}}
  {{/each ~}}
}
