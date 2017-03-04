use format::field::*;
use nom::{be_u8,be_u16,be_u32,be_u64};

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
    use format::field::*;
    use nom::{be_u8,be_u16,be_u32,be_u64};

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
          pub {{snake argument.name}}: {{map_type argument}},
        {{/each ~}}

      }

      named!(parse_{{snake method.name}}<{{camel method.name}}>,
        do_parse!(
          {{#each method.arguments as |argument| ~}}
            {{snake argument.name}}: {{map_parser argument}} >>
          {{/each ~}}

          ({{camel method.name}} {
            {{#each method.arguments as |argument| ~}}
              {{snake argument.name}}: {{snake argument.name}},
            {{/each ~}}
          })
        )
      );
    {{/each}}
  }
{{/each}}
