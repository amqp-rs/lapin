pub struct {{camel_name}} {

}

named!(parse_{{snake_name}}<{{camel_name}}>,
  value!({{camel_name}} {} )
);
/*
{{id}} - {{name}}
synchronous: {{synchronous}}
{{arguments}}
*/
