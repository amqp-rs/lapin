pub const DESCRIPTION: &'static str = "{{name}} - {{major_version}}.{{minor_version}}.{{revision}}";

pub enum Class {
  {{#each specs.classes as |class| ~}}
    {{class.name}},
  {{/each ~}}
  None,
}

named!(pub parse<Class>,
  value!(Class::None)
);

{{#each specs.classes as |class|}}
  pub mod {{class.name}} {
    use super::Class;

    pub enum Methods {
      {{#each class.methods as |method| ~}}
        {{method.name}},
      {{/each ~}}
    }

    named!(pub parse<Class>,
      value!(Class::None)
    );

    {{#each class.methods as |method|}}
      pub struct {{method.name}} {
        {{#each method.arguments as |argument| ~}}
          pub {{argument.name}}: {{(argument.amqp_type)}},
        {{/each ~}}

      }

      named!(parse_{{method.name}}<{{method.name}}>,
        value!({{method.name}} {} )
      );
    {{/each}}
  }
{{/each}}
