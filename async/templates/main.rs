use amq_protocol::types::*;
use amq_protocol::types::flags::*;
use amq_protocol::types::generation::*;
use amq_protocol::types::parsing::*;
use cookie_factory::*;
use nom::{be_i8,be_i16,be_i32,be_i64,be_u8,be_u16,be_u32,be_u64};

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

pub fn gen_class<'a>(input:(&'a mut [u8],usize), class: &Class) -> Result<(&'a mut [u8],usize),GenError> {
  match class {
    {{#each specs.classes as |class| ~}}
    &Class::{{camel class.name}}(ref {{snake class.name}}) => {
      {{snake class.name}}::gen(input, {{snake class.name}})
    },
    {{/each ~}}
    &Class::None => Err(GenError::CustomError(14)),
  }
}

{{#each specs.classes as |class|}}
  pub mod {{snake class.name}} {
    use super::Class;
    use amq_protocol::types::*;
    use amq_protocol::types::flags::*;
    use amq_protocol::types::generation::*;
    use amq_protocol::types::parsing::*;
    use cookie_factory::*;
    use nom::{be_i8,be_i16,be_i32,be_i64,be_u8,be_u16,be_u32,be_u64};

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

    pub fn gen<'a>(input:(&'a mut [u8],usize), method: &Methods) -> Result<(&'a mut [u8],usize),GenError> {
      match method {
        {{#each class.methods as |method| ~}}
        &Methods::{{camel method.name}}(ref {{snake method.name}}) => {
          do_gen!(input,
            gen_be_u16!({{class.id}}u16) >>
            gen_{{snake method.name}}({{snake method.name}})
          )
        },
        {{/each ~}}
        &Methods::None => Err(GenError::CustomError(15)),
      }
    }

    {{#each class.methods as |method|}}
      #[derive(Clone,Debug,PartialEq)]
      pub struct {{camel method.name}} {
        {{#each_argument method.arguments as |argument| ~}}
          {{#if argument_is_value ~}}
            pub {{snake argument.name}}: {{argument.type}},
          {{else}}
            {{#each_flag argument as |flag| ~}}
              pub {{snake flag.name}}: Boolean,
            {{/each_flag ~}}
          {{/if ~}}
        {{/each_argument ~}}
      }

      named!(parse_{{snake method.name}}<Methods>,
        do_parse!(
          {{#each_argument method.arguments as |argument| ~}}
            {{#if argument_is_value ~}}
              {{snake argument.name}} : parse_{{snake_type argument.type}} >>
            {{else}}
              flags : apply!(parse_flags, &vec![
                {{#each_flag argument as |flag| ~}}
                  "{{flag.name}}",
                {{/each_flag ~}}
              ]) >>
            {{/if ~}}
          {{/each_argument ~}}

          (Methods::{{camel method.name}}({{camel method.name}} {
            {{#each_argument method.arguments as |argument| ~}}
              {{#if argument_is_value ~}}
                {{snake argument.name}}: {{snake argument.name}},
              {{else}}
                {{#each_flag argument as |flag| ~}}
                  {{snake flag.name}}: flags.get_flag("{{snake flag.name}}").unwrap_or({{flag.default_value}}),
                {{/each_flag ~}}
              {{/if ~}}
            {{/each_argument ~}}
          }))
        )
      );

      pub fn gen_{{snake method.name}}<'a>(input:(&'a mut [u8],usize), method: &{{camel method.name}}) -> Result<(&'a mut [u8],usize),GenError> {
        {{#if method.has_flags ~}}
          let mut flags = AMQPFlags::new();
          {{#each_argument method.arguments as |argument| ~}}
            {{#unless argument_is_value ~}}
              {{#each_flag argument as |flag| ~}}
                flags.add_flag("{{snake flag.name}}".to_string(), method.{{snake flag.name}});
              {{/each_flag ~}}
            {{/unless ~}}
          {{/each_argument ~}}
        {{/if ~}}
        do_gen!(input,
          gen_be_u16!({{method.id}}u16)
          {{#each_argument method.arguments as |argument| ~}}
            {{#if argument_is_value ~}}
              >> gen_{{snake_type argument.type}}(&method.{{snake argument.name}})
            {{else}}
              >> gen_flags(&flags)
            {{/if ~}}
          {{/each_argument ~}}
        )
      }
    {{/each}}
  }
{{/each}}
