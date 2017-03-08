use amq_protocol_types::*;
use format::field::*;
use format::frame::*;
use connection::*;
use generated::*;

impl Connection {

  {{#each specs.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  pub fn {{snake class.name}}_{{snake method.name}}(&mut self,
    _channel_id: u16{{#each method.arguments as |argument| ~}},
    {{snake argument.name}}: {{argument.type}}{{/each ~}}) {
      let method = Class::{{camel class.name}}({{snake class.name}}::Methods::{{camel method.name}} (
        {{snake class.name}}::{{camel method.name}} {
          {{#each method.arguments as |argument| ~}}
          {{snake argument.name}}: {{snake argument.name}},
          {{/each ~}}
        }
      ));

    self.send_method_frame(_channel_id, &method);
  }

  {{/each ~}}
{{/each ~}}
}

