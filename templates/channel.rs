pub mod options {
  use super::*;

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.ignore_args ~}}
  {{#each_argument method.arguments as |argument| ~}}
  {{#unless @argument_is_value ~}}
  {{#unless argument.ignore_flags ~}}
  #[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
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
  {{camel class.name}}{{camel method.name}}Ok(PromiseResolver<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}>{{#each method.metadata.state as |state| ~}}, {{state.type}}{{/each ~}}),
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
        error!("the client should not receive this method: {:?}", m);
        self.handle_invalid_contents(format!("unexepcted method received on channel {}", self.id), m.get_amqp_class_id(), m.get_amqp_method_id())
      }
    }
  }

  {{#each protocol.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  {{#unless method.metadata.skip ~}}
  {{#if method.c2s ~}}
#[allow(clippy::too_many_arguments)]
{{include_more class.name method.name}}{{#unless method.metadata.require_wrapper ~}}{{#if method.is_reply ~}}{{#if method.metadata.internal ~}}pub(crate) {{/if ~}}{{else}}pub {{#if method.metadata.internal ~}}(crate) {{/if ~}}{{/if ~}}fn {{else}}fn do_{{/unless ~}}{{snake class.name false}}_{{snake method.name false}}(&self{{#unless method.ignore_args ~}}{{#each_argument method.arguments as |argument| ~}}{{#if @argument_is_value ~}}{{#unless argument.force_default ~}}, {{snake argument.name}}: {{#if (use_str_ref argument.type) ~}}&str{{else}}{{argument.type}}{{/if ~}}{{/unless ~}}{{else}}{{#unless argument.ignore_flags ~}}, options: {{camel class.name}}{{camel method.name}}Options{{/unless ~}}{{/if ~}}{{/each_argument ~}}{{/unless ~}}{{#each method.metadata.extra_args as |arg| ~}}, {{arg.name}}: {{arg.type}}{{/each ~}}) -> Promise{{#if method.metadata.confirmation.type ~}}Chain{{/if ~}}<{{#if method.metadata.confirmation.type ~}}{{method.metadata.confirmation.type}}{{else}}(){{/if ~}}> {
    {{#if method.metadata.channel_init ~}}
    if !self.status.initializing() {
    {{else}}
    {{#if method.metadata.channel_deinit ~}}
    if !self.status.closing() {
    {{else}}
    if !self.status.connected() {
    {{/if ~}}
    {{/if ~}}
      return Promise{{#if method.metadata.confirmation.type ~}}Chain{{/if ~}}::new_with_data(Err(Error::InvalidChannelState(self.status.state())));
    }

    {{#if method.metadata.start_hook ~}}
    {{#if method.metadata.start_hook.returns ~}}let start_hook_res = {{/if ~}}self.before_{{snake class.name false}}_{{snake method.name false}}({{#each method.metadata.start_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}});
    {{/if ~}}

    {{#each method.metadata.init_clones as |init_clone| ~}}
    let {{init_clone.to}} = {{init_clone.from}}.clone();
    {{/each ~}}
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

    {{#if method.metadata.carry_headers ~}}
    self.send_method_frame_with_body(method, payload, properties, start_hook_res).unwrap_or_else(|err| PromiseChain::new_with_data(Err(err)))
    {{else}}
    let (promise, send_resolver) = Promise::new();
    if log_enabled!(Trace) {
      promise.set_marker("{{class.name}}.{{method.name}}".into());
    }
    {{#if method.synchronous ~}}
    let (promise, resolver) = Promise{{#if method.metadata.confirmation.type ~}}Chain{{/if ~}}::after(promise);
    if log_enabled!(Trace) {
      promise.set_marker("{{class.name}}.{{method.name}}.Ok".into());
    }
    {{/if ~}}
    self.send_method_frame(method, send_resolver, {{#if method.synchronous ~}}Some(ExpectedReply(Reply::{{camel class.name}}{{camel method.name}}Ok(resolver.clone(){{#each method.metadata.state as |state| ~}}, {{#if state.provider}}{{state.provider}}{{else}}{{state.name}}{{#if state.use_str_ref ~}}.into(){{/if ~}}{{/if ~}}{{/each ~}}), Box::new(resolver))){{else}}None{{/if ~}});
    {{#if method.metadata.end_hook ~}}
    let end_hook_res = self.on_{{snake class.name false}}_{{snake method.name false}}_sent({{#each method.metadata.end_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}});
    if let Err(err) = end_hook_res {
      return Promise{{#if method.metadata.confirmation.type ~}}Chain{{/if ~}}::new_with_data(Err(err));
    }
    {{/if ~}}

    {{#if method.synchronous ~}}
    {{#if method.metadata.nowait_hook ~}}
    if nowait {
      if let Err(err) = self.receive_{{snake class.name false}}_{{snake method.name false}}_ok(protocol::{{snake class.name}}::{{camel method.name}}Ok { {{#each method.metadata.nowait_hook.fields as |field| ~}}{{field}}, {{/each ~}}{{#if method.metadata.nowait_hook.nonexhaustive_args ~}}..Default::default(){{/if ~}} }) {
        return Promise{{#if method.metadata.confirmation.type ~}}Chain{{/if ~}}::new_with_data(Err(err));
      }
    }
    {{/if ~}}
    {{/if ~}}
    promise
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
      return Err(Error::InvalidChannelState(self.status.state()));
    }

    match self.frames.next_expected_reply(self.id) {
      Some(Reply::{{camel class.name}}{{camel method.name}}(resolver{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}})) => {
        {{#unless method.metadata.confirmation.type ~}}let res ={{/unless ~}}
        {{#if method.arguments ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received(method{{#if method.metadata.confirmation.type ~}}, resolver{{/if ~}}{{#each method.metadata.state as |state| ~}}, {{state.name}}{{/each ~}})
        {{else}}
        {{#if method.metadata.received_hook ~}}
        self.on_{{snake class.name false}}_{{snake method.name false}}_received({{#each method.metadata.received_hook.params as |param| ~}}{{#unless @first ~}}, {{/unless ~}}{{param}}{{/each ~}})
        {{else}}
        Ok(())
        {{/if ~}}
        {{/if ~}}
        {{#unless method.metadata.confirmation.type ~}};
        resolver.swear(res.clone());
        res
        {{/unless ~}}
      },
      _ => {
        self.handle_invalid_contents(format!("unexepcted {{class.name}} {{method.name}} received on channel {}", self.id), method.get_amqp_class_id(), method.get_amqp_method_id())
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
      return Err(Error::InvalidChannelState(self.status.state()));
    }
    self.on_{{snake class.name false}}_{{snake method.name false}}_received(method)
  }
  {{/if ~}}
  {{/if ~}}
  {{/unless ~}}
  {{/each ~}}
  {{/each ~}}
}
