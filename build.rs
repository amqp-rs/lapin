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
    let dest_path2   = Path::new(&out_dir).join("api.rs");
    let mut f        = File::create(&dest_path).expect("Failed to create generated.rs");
    let mut f2       = File::create(&dest_path2).expect("Failed to create api.rs");
    let mut full_tpl = String::new();
    let mut api_tpl = String::new();
    std::fs::File::open("templates/main.rs").expect("Failed to open main template").read_to_string(&mut full_tpl).expect("Failed to read main template");
    std::fs::File::open("templates/api.rs").expect("Failed to open main template").read_to_string(&mut api_tpl).expect("Failed to read main template");

    let mut handlebars = Handlebars::new();
    let mut data = BTreeMap::new();

    handlebars.register_escape_fn(handlebars::no_escape);
    handlebars.register_helper("snake", Box::new(snake_helper));
    handlebars.register_helper("camel", Box::new(camel_helper));
    handlebars.register_helper("map_type", Box::new(map_type_helper));
    handlebars.register_helper("map_parser", Box::new(map_parser_helper));
    handlebars.register_helper("map_generator", Box::new(map_generator_helper));

    handlebars.register_template_string("full", full_tpl).expect("Failed to register full template");
    handlebars.register_template_string("api", api_tpl).expect("Failed to register api template");

    let specs     = AMQProtocolDefinition::load();
    data.insert("specs".to_string(), specs);
    writeln!(f, "{}", handlebars.render("full", &data).expect("Failed to render full template"));
    writeln!(f2, "{}", handlebars.render("api", &data).expect("Failed to render full template"));

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
  println!("val1: {:?}", h.param(0));
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val.clone()).unwrap();

  let rendered = match arg.amqp_type {
    Some(AMQPType::Bit)       => "bool",
    Some(AMQPType::Octet)     => "u8",
    Some(AMQPType::Short)     => "u16",
    Some(AMQPType::Long)      => "u32",
    Some(AMQPType::LongLong)  => "u64",
    Some(AMQPType::ShortStr)  => "String",
    Some(AMQPType::LongStr)   => "String",
    Some(AMQPType::Table)     => "::std::collections::HashMap<String,Value>",
    Some(AMQPType::Timestamp) => "u64",
    None                      => {
      let data:  serde_json::Map<String,  serde_json::Value> = serde_json::from_value(rc.context().data().clone()).unwrap();
      let specs: serde_json::Map<String, serde_json::Value> =  serde_json::from_value(data.get("specs").unwrap().clone()).unwrap();
      let domains: Vec<AMQPDomain> =  serde_json::from_value(specs.get("domains").unwrap().clone()).unwrap();

      let lookup_domain = arg.domain.clone().unwrap();
      let res = domains.iter().find(|d| d.0 == lookup_domain).map(|d| match d.1 {
        AMQPType::Bit       => "bool",
        AMQPType::Octet     => "u8",
        AMQPType::Short     => "u16",
        AMQPType::Long      => "u32",
        AMQPType::LongLong  => "u64",
        AMQPType::ShortStr  => "String",
        AMQPType::LongStr   => "String",
        AMQPType::Table     => "::std::collections::HashMap<String,Value>",
        AMQPType::Timestamp => "u64",
      }).unwrap();
      println!("key {} => domain gave type: {}", arg.name, res);
      res
    },
  };
  try!(rc.writer.write(rendered.as_bytes()));

  Ok(())
}

fn map_parser_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  println!("val1: {:?}", h.param(0));
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val).unwrap();

  let rendered = match arg.amqp_type {
    Some(AMQPType::Bit)       => "map!(be_u8, |u| u != 0)",
    Some(AMQPType::Octet)     => "be_u8",
    Some(AMQPType::Short)     => "be_u16",
    Some(AMQPType::Long)      => "be_u32",
    Some(AMQPType::LongLong)  => "be_u64",
    Some(AMQPType::ShortStr)  => "map!(short_string, |s:&str| s.to_string())",
    Some(AMQPType::LongStr)   => "map!(long_string,  |s:&str| s.to_string())",
    Some(AMQPType::Table)     => "field_table",
    Some(AMQPType::Timestamp) => "be_u64",
    None                      => {
      let data:  serde_json::Map<String,  serde_json::Value> = serde_json::from_value(rc.context().data().clone()).unwrap();
      let specs: serde_json::Map<String, serde_json::Value> =  serde_json::from_value(data.get("specs").unwrap().clone()).unwrap();
      let domains: Vec<AMQPDomain> =  serde_json::from_value(specs.get("domains").unwrap().clone()).unwrap();

      let lookup_domain = arg.domain.clone().unwrap();
      domains.iter().find(|d| d.0 == lookup_domain).map(|d| match d.1 {
        AMQPType::Bit       => "map!(be_u8, |u| u != 0)",
        AMQPType::Octet     => "be_u8",
        AMQPType::Short     => "be_u16",
        AMQPType::Long      => "be_u32",
        AMQPType::LongLong  => "be_u64",
        AMQPType::ShortStr  => "map!(short_string, |s:&str| s.to_string())",
        AMQPType::LongStr   => "map!(long_string,  |s:&str| s.to_string())",
        AMQPType::Table     => "field_table",
        AMQPType::Timestamp => "be_u64",
      }).unwrap()
    }
  };
  try!(rc.writer.write(rendered.as_bytes()));

  Ok(())
}

fn map_generator_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  println!("val1: {:?}", h.param(0));
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val).unwrap();

  let rendered = match arg.amqp_type {
    Some(AMQPType::Bit)       => "gen_bool",
    Some(AMQPType::Octet)     => "gen_be_u8",
    Some(AMQPType::Short)     => "gen_be_u16",
    Some(AMQPType::Long)      => "gen_be_u32",
    Some(AMQPType::LongLong)  => "gen_be_u64",
    Some(AMQPType::ShortStr)  => "gen_short_string",
    Some(AMQPType::LongStr)   => "gen_long_string",
    Some(AMQPType::Table)     => "gen_field_table",
    Some(AMQPType::Timestamp) => "gen_be_u64",
    None                      => {
      let data:  serde_json::Map<String,  serde_json::Value> = serde_json::from_value(rc.context().data().clone()).unwrap();
      let specs: serde_json::Map<String, serde_json::Value> =  serde_json::from_value(data.get("specs").unwrap().clone()).unwrap();
      let domains: Vec<AMQPDomain> =  serde_json::from_value(specs.get("domains").unwrap().clone()).unwrap();

      let lookup_domain = arg.domain.clone().unwrap();
      domains.iter().find(|d| d.0 == lookup_domain).map(|d| match d.1 {
        AMQPType::Bit       => "gen_bool",
        AMQPType::Octet     => "gen_be_u8",
        AMQPType::Short     => "gen_be_u16",
        AMQPType::Long      => "gen_be_u32",
        AMQPType::LongLong  => "gen_be_u64",
        AMQPType::ShortStr  => "gen_short_string",
        AMQPType::LongStr   => "gen_long_string",
        AMQPType::Table     => "gen_field_table",
        AMQPType::Timestamp => "gen_be_u64",
      }).unwrap()
    }
  };
  try!(rc.writer.write(rendered.as_bytes()));

  Ok(())
}
