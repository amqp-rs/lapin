use format::field::*;
use format::frame::*;
use connection::*;
use generated::*;

impl Connection {

{{#each specs.classes as |class| ~}}
  {{#each class.methods as |method| ~}}
  pub fn {{snake class.name}}_{{snake method.name}}(&mut self, _channel_id: u16
  {{#each method.arguments as |argument| ~}} ,{{snake argument.name}}: {{map_type argument}}{{/each ~}}) {
    let method = Class::{{camel class.name}}({{snake class.name}}::Methods::{{camel method.name}} (
      {{snake class.name}}::{{camel method.name}} {
      {{#each method.arguments as |argument| ~}}
        {{snake argument.name}}: {{snake argument.name}},
      {{/each ~}}
      }
    ));

    match gen_method_frame((&mut self.send_buffer.space(), 0), _channel_id, &method).map(|tup| tup.1) {
      Ok(sz) => {
        self.send_buffer.fill(sz);
      },
      Err(e) => {
        println!("error generating frame: {:?}", e);
        self.state = ConnectionState::Error;
      }
    }
  }

  {{/each ~}}
{{/each ~}}
}

