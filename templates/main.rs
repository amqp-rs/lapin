pub const DESCRIPTION: &'static str = "{{name}} - {{major_version}}.{{minor_version}}.{{revision}}";

pub enum Class {
  {{#each specs.classes as |class| ~}}
    {{camel class.name}},
  {{/each ~}}
  None,
}

named!(pub parse<Class>,
  value!(Class::None)
);

{{#each specs.classes as |class|}}
  pub mod {{camel class.name}} {
    use super::Class;

    pub enum Methods {
      {{#each class.methods as |method| ~}}
        {{camel method.name}},
      {{/each ~}}
    }

    named!(pub parse<Class>,
      value!(Class::None)
    );

    {{#each class.methods as |method|}}
      pub struct {{camel method.name}} {
        {{#each method.arguments as |argument| ~}}
          pub {{camel argument.name}}: {{map_type (argument.amqp_type)}},
        {{/each ~}}

      }

      named!(parse_{{snake method.name}}<{{camel method.name}}>,
        value!({{camel method.name}} {
          {{#each method.arguments as |argument| ~}}
            {{camel argument.name}}: (),
          {{/each ~}}
        } )
      );
    {{/each}}
  }
{{/each}}
