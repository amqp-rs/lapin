pub mod {{snake_name}} {
  use super::Class;

  pub enum Methods {
  }

  named!(pub parse<Class>,
    value!(Class::None)
  );

  {{methods}}
}
/*
{{id}} - {{name}}
{{methods}}

{{properties}}
*/
