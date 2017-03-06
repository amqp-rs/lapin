use format::field::*;
use rusticata_macros::*;
use nom::{be_u8,be_u16,be_u32,be_u64};

pub const DESCRIPTION: &'static str = "{{name}} - {{major_version}}.{{minor_version}}.{{revision}}";

#[derive(Clone,Debug,PartialEq)]
pub enum Class {
  {{#each specs.classes as |class| ~}}
    {{camel class.name}}({{snake class.name}}::Methods),
  {{/each ~}}
  None,
}

macro_rules! call_path (
  ($i: expr, $p: path) => ($p($i))
);

named!(pub parse_class<Class>,
  switch!(be_u16,
  {{#each specs.classes as |class| ~}}
    {{class.id}} => call_path!({{snake class.name}}::parse) |
  {{/each ~}}
  _  => value!(Class::None)
  )
);

{{#each specs.classes as |class|}}
  pub mod {{snake class.name}} {
    use super::Class;
    use format::field::*;
    use rusticata_macros::*;
    use nom::{be_u8,be_u16,be_u32,be_u64};

    #[derive(Clone,Debug,PartialEq)]
    pub enum Methods {
      {{#each class.methods as |method| ~}}
        {{camel method.name}}({{camel method.name}}),
      {{/each ~}}
      None,
    }

    named!(pub parse<Class>,
      switch!(be_u16,
        {{#each class.methods as |method| ~}}
        {{method.id}} => map!(call!(parse_{{snake method.name}}), |m| Class::{{camel class.name}}(m)) |
        {{/each ~}}
        _  => value!(Class::{{camel class.name}}(Methods::None))
      )
    );

    {{#each class.methods as |method|}}
      #[derive(Clone,Debug,PartialEq)]
      pub struct {{camel method.name}} {
        {{#each method.arguments as |argument| ~}}
          pub {{snake argument.name}}: {{map_type argument}},
        {{/each ~}}

      }

      named!(parse_{{snake method.name}}<Methods>,
        do_parse!(
          {{#each method.arguments as |argument| ~}}
            {{snake argument.name}}: {{map_parser argument}} >>
          {{/each ~}}

          (Methods::{{camel method.name}}({{camel method.name}} {
            {{#each method.arguments as |argument| ~}}
              {{snake argument.name}}: {{snake argument.name}},
            {{/each ~}}
          }))
        )
      );
      pub fn gen_{{snake method.name}}<'a>(x:(&'a mut [u8],usize), method: &{{camel method.name}}) -> Result<(&'a mut [u8],usize),GenError> {
        do_gen!(x,
          gen_be_u16!({{method.id}}u16)
          {{#each method.arguments as |argument| ~}}
          >> {{map_generator argument}} (&method.{{snake argument.name}})
          {{/each ~}}
        )
      }
    {{/each}}
  }
{{/each}}
