extern crate amq_protocol_codegen;
extern crate handlebars;
extern crate serde_json;

use amq_protocol_codegen::*;
use handlebars::{Handlebars,Helper,HelperDef,RenderContext,RenderError};

use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::ascii::AsciiExt;

fn main() {
    let out_dir      = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let dest_path    = Path::new(&out_dir).join("generated.rs");
    let mut f        = File::create(&dest_path).expect("Failed to create generated.rs");
    let mut main_tpl = String::new();
    let mut full_tpl = String::new();
    std::fs::File::open("templates/main.rs").expect("Failed to open main template").read_to_string(&mut full_tpl).expect("Failed to read main template");

    let mut handlebars = Handlebars::new();
    let mut data = BTreeMap::new();

    handlebars.register_escape_fn(handlebars::no_escape);
    handlebars.register_helper("snake", Box::new(snake_helper));
    handlebars.register_helper("camel", Box::new(camel_helper));
    handlebars.register_helper("map_type", Box::new(map_type_helper));

    handlebars.register_template_string("full", full_tpl).expect("Failed to register full template");

    let specs     = AMQProtocolDefinition::load();
    data.insert("specs".to_string(), specs);
    writeln!(f, "{}", handlebars.render("full", &data).expect("Failed to render full template"));

    //writeln!(f, "{}", specs.codegen_full(&full_tpl)).expect("Failed to generate generated.rs");
}

fn camel_name(name: &str) -> String {
    let mut new_word: bool = true;
    name.chars().fold("".to_string(), |mut result, ch| {
        if ch == '-' || ch == '_' || ch == ' ' {
            new_word = true;
            result
        } else {
            result.push(if new_word { ch.to_ascii_uppercase() } else { ch.to_ascii_lowercase() });
            new_word = false;
            result
        }
    })
}

fn snake_name(name: &str) -> String {
    match name {
        "type"   => "amqp_type".to_string(),
        "return" => "amqp_return".to_string(),
        name     => name.replace("-", "_"),
    }
}

fn camel_helper (h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
    // just for example, add error check for unwrap
    let param = h.param(0).unwrap().value().as_str().unwrap();
    let rendered = camel_name(param);
    try!(rc.writer.write(rendered.into_bytes().as_ref()));
    Ok(())
}

fn snake_helper (h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
    // just for example, add error check for unwrap
    let param = h.param(0).unwrap().value().as_str().unwrap();
    let rendered = snake_name(param);
    try!(rc.writer.write(rendered.into_bytes().as_ref()));
    Ok(())
}

fn map_type_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  let val = h.param(0).unwrap().value().clone();
  println!("val: {:?}", val);
  let value: Option<AMQPType> = if val == serde_json::Value::String("".to_string()) {
    None
  } else {
    serde_json::from_value(val).unwrap()
  };
  let rendered = match value {
    Some(AMQPType::Bit)       => "bool",
    Some(AMQPType::Octet)     => "u8",
    Some(AMQPType::Short)     => "u16",
    Some(AMQPType::Long)      => "u32",
    Some(AMQPType::LongLong)  => "u64",
    Some(AMQPType::ShortStr)  => "String",
    Some(AMQPType::LongStr)   => "String",
    Some(AMQPType::Table)     => "String",
    Some(AMQPType::Timestamp) => "u64",
    None                      => "()",
  };
  try!(rc.writer.write(rendered.as_bytes()));
  Ok(())
}
