/// Options structs for every AMQP method that accepts flag arguments.
pub mod options {
  use super::*;

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.ignore_args ~}}
  {{#each_argument method.arguments as |argument| ~}}
  {{#unless @argument_is_value ~}}
  {{#unless argument.ignore_flags ~}}
  /// Options for the `{{class.name}}.{{method.name}}` AMQP method.
  #[derive(Copy, Clone, Debug, Default, PartialEq)]
  pub struct {{camel class.name}}{{camel method.name}}Options {
    {{#each argument.flags as |flag| ~}}
    /// {{lookup @root.protocol.metadata.flags_doc flag.name}}
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
  ConnectionStep(ConnectionStep),
  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#if method.c2s ~}}
  {{#if method.synchronous ~}}
  {{#unless method.metadata.connection_step ~}}
  {{camel class.name}}{{camel method.name}}Ok(PromiseResolver<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}>{{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{state.type}}{{/each ~}}{{/if ~}}),
  {{/unless ~}}
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
{{include_more class.name method.name}}{{#unless method.metadata.require_wrapper ~}}{{#if method.is_reply ~}}{{#if method.metadata.internal ~}}pub(crate) {{/if ~}}{{else}}pub {{#if method.metadata.internal ~}}(crate) {{/if ~}}{{/if ~}}async fn {{else}}async fn do_{{/unless ~}}{{snake class.name false}}_{{snake method.name false}}(&self{{#unless method.ignore_args ~}}{{#each_argument method.arguments as |argument| ~}}{{#if @argument_is_value ~}}{{#unless argument.force_default ~}}, {{snake argument.name}}: {{argument.type}}{{/unless ~}}{{else}}{{#unless argument.ignore_flags ~}}, options: {{camel class.name}}{{camel method.name}}Options{{/unless ~}}{{/if ~}}{{/each_argument ~}}{{/unless ~}}{{#if method.metadata.extra_args ~}}{{#each method.metadata.extra_args as |arg| ~}}, {{arg.name}}: {{arg.type}}{{/each ~}}{{/if ~}}) -> Result<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}> {
    {{#unless class.metadata.channel0_only ~}}
    {{#if method.metadata.channel_init ~}}
    if !self.status.initializing() {
    {{else if method.metadata.channel_deinit ~}}
    if !self.status.closing() {
    {{else if method.metadata.channel_recovery ~}}
    if !self.status.connected_or_recovering() {
    {{else}}
    if !self.status.connected() {
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
    {{#unless method.metadata.carry_headers ~}}
    let (promise, resolver) = Promise::new("{{class.name}}.{{method.name}}");
    {{/unless ~}}
    {{#if method.synchronous ~}}
    {{#if method.metadata.connection_step ~}}
    let reply = Reply::ConnectionStep(ConnectionStep::{{camel method.name}}({{#each method.metadata.extra_args as |arg| ~}}{{#unless @first ~}}, {{/unless ~}}{{arg.name}}{{/each ~}}));
    {{else}}
    let reply = Reply::{{camel class.name}}{{camel method.name}}Ok(resolver.clone(){{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{#if state.provider}}{{state.provider}}{{else}}{{state.name}}{{#if state.clone ~}}.clone(){{/if ~}}{{/if ~}}{{/each ~}}{{/if ~}});
    {{/if ~}}
    {{#if method.metadata.nowait_hook ~}}
    let nowait_reply = {{#if method.metadata.nowait_hook.fields ~}}nowait.then(|| {{else}}nowait.then_some({{/if ~}}protocol::{{snake class.name}}::{{camel method.name}}Ok { {{#if method.metadata.nowait_hook.fields ~}}{{#each method.metadata.nowait_hook.fields as |field| ~}}{{field}}: {{field}}.clone(), {{/each ~}}{{/if ~}}{{#if method.metadata.nowait_hook.nonexhaustive_args ~}}..Default::default(){{/if ~}} });
    {{/if ~}}
    {{/if ~}}
    let method = AMQPClass::{{camel class.name}}(protocol::{{snake class.name}}::AMQPMethod::{{camel method.name}} (protocol::{{snake class.name}}::{{camel method.name}} {
      {{#each_argument method.arguments as |argument| ~}}
      {{#if @argument_is_value ~}}
      {{#unless argument.force_default ~}}
      {{snake argument.name}},
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

    {{#if method.metadata.carry_headers ~}}
    self.send_method_frame_with_body("{{class.name}}.{{method.name}}", method, payload, properties, start_hook_res).await
    {{else}}
    {{#if method.metadata.resolver_hook ~}}{{method.metadata.resolver_hook}}{{/if ~}}
    self.send_method_frame(method, Box::new(resolver.clone()), {{#if method.synchronous ~}}Some(ExpectedReply(reply, Box::new(resolver))), None{{else if method.metadata.connection_step ~}}Some(ExpectedReply(Reply::ConnectionStep(ConnectionStep::{{camel method.name}}({{#each method.metadata.extra_args as |arg| ~}}{{#unless @first ~}}, {{/unless ~}}{{arg.name}}{{/each ~}})), Box::new(resolver))), None{{else}}None, Some(resolver){{/if ~}});
    {{#if method.metadata.end_hook ~}}
    self.on_{{snake class.name false}}_{{snake method.name false}}_sent({{#if method.metadata.end_hook.params ~}}{{#each method.metadata.end_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}}{{/if ~}});
    {{/if ~}}

    {{#if method.synchronous ~}}
    {{#if method.metadata.nowait_hook ~}}
    if let Some(nowait_reply) = nowait_reply {
      self.receive_{{snake class.name false}}_{{snake method.name false}}_ok(nowait_reply)?;
    }
    {{/if ~}}
    {{/if ~}}
    promise.await
    {{/if ~}}
  }
  {{/if ~}}

  {{#if method.s2c ~}}
  fn receive_{{snake class.name false}}_{{snake method.name false}}(&self, method: protocol::{{snake class.name}}::{{camel method.name}}) -> Result<()> {
  {{#if method.is_reply ~}}
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

    match {{#if method.metadata.expected_reply_getter ~}}{{method.metadata.expected_reply_getter}}{{else if method.metadata.connection_step ~}}self.frames.find_connection_step(self.id){{else}}self.frames.find_expected_reply(self.id, |reply| matches!(&reply.0, Reply::{{camel class.name}}{{camel method.name}}(..))){{/if ~}} {
      {{#if method.metadata.connection_step ~}}
      Some(ConnectionStep::{{method.metadata.connection_step}}) => {
      {{else}}
      Some(Reply::{{camel class.name}}{{camel method.name}}(resolver{{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}}{{/if ~}})) => {
      {{/if ~}}
        {{#unless method.metadata.confirmation.type ~}}let res ={{/unless ~}}
        {{#if method.arguments ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received(method{{#if method.metadata.confirmation.type ~}}, resolver{{/if ~}}{{#if method.metadata.state ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}}{{/if ~}})
        {{else if method.metadata.received_hook ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received({{#if method.metadata.received_hook.params ~}}{{#each method.metadata.received_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}}{{/if ~}})
        {{else}}
        Ok(())
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
  {{else if method.metadata.connection_step ~}}
    self.assert_channel0(
      method.get_amqp_class_id(),
      method.get_amqp_method_id(),
    )?;
    if !self.status.can_receive_messages() {
      return Err(self.status.state_error("{{class.name}}.{{method.name}}"));
    }

    match self.frames.find_connection_step(self.id) {
      Some(step) => self.on_{{snake class.name false}}_{{snake method.name false}}_received(method, step),
      None => self.connection_process_error(self.connection_status.state(), None, None),
    }
  {{else}}
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
  {{/if ~}}
  }
  {{/if ~}}
  {{/unless ~}}
  {{/each ~}}
  {{/each ~}}
}
