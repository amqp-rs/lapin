extern crate amq_protocol_codegen;
extern crate amq_protocol_types;
extern crate handlebars;
extern crate serde_json;

use amq_protocol_codegen::*;
use amq_protocol_types::*;
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

fn get_type_and_name(arg: &AMQPArgument, specs: &serde_json::Map<String, serde_json::Value>) -> (AMQPType, String) {
  match arg.amqp_type {
    Some(ref ty) => (ty.clone(), arg.name.to_string()),
    None     => {
      let domains: Vec<AMQPDomain> = serde_json::from_value(specs.get("domains").unwrap().clone()).unwrap();

      let arg_domain = arg.domain.as_ref().map(|d| d.to_string()).unwrap();
      let res = domains.iter().find(|d| d.name == arg_domain).map(|d| d.amqp_type.clone()).unwrap();
      //println!("key {} => domain gave type: {}", arg.name, res);
      (res, arg_domain.to_string())
    },
  }
}

fn map_type_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  println!("val1: {:?}", h.param(0));
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val.clone()).unwrap();

  let rendered = match arg.amqp_type {
    Some(ty) => ty.to_string(),
    None     => {
      let specs = get_specs(rc);
      let domains: Vec<AMQPDomain> = serde_json::from_value(specs.get("domains").unwrap().clone()).unwrap();

      let lookup_domain = arg.domain.clone().unwrap();
      let res = domains.iter().find(|d| d.name == lookup_domain).map(|d| d.amqp_type.to_string()).unwrap();
      println!("key {} => domain gave type: {}", arg.name, res);
      res
    },
  };
  try!(rc.writer.write(rendered.as_bytes()));

  Ok(())
}

fn get_specs(rc: &mut RenderContext) ->  serde_json::Map<String, serde_json::Value> {
  let data:  serde_json::Map<String, serde_json::Value> = serde_json::from_value(rc.context().data().clone()).unwrap();
  let specs: serde_json::Map<String, serde_json::Value> = serde_json::from_value(data.get("specs").unwrap().clone()).unwrap();

  specs
}

fn get_arguments(h: &Helper, rc: &mut RenderContext, class_id_index: usize, method_id_index: usize) -> Vec<AMQPArgument> {
  let class_id:  u8 = serde_json::from_value(h.param(class_id_index).unwrap().value().clone()).unwrap();
  let method_id: u8 = serde_json::from_value(h.param(method_id_index).unwrap().value().clone()).unwrap();

  let specs = get_specs(rc);

  let classes: Vec<AMQPClass> = serde_json::from_value(specs.get("classes").unwrap().clone()).unwrap();

  let arguments:Vec<AMQPArgument> = classes.iter().find(|c| c.id == class_id).and_then(|class| {
    class.methods.iter().find(|m| m.id == method_id)
  }).map(|m| m.arguments.clone()).unwrap();

  arguments
}
fn map_parser_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  println!("val1: {:?}", h.param(0));
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val).unwrap();

  let rendered = match arg.amqp_type {
    Some(AMQPType::Boolean)        => "map!(be_u8, |u| u != 0)",
    Some(AMQPType::ShortShortInt)  => "be_i8",
    Some(AMQPType::ShortShortUInt) => "be_u8",
    Some(AMQPType::ShortInt)       => "be_i16",
    Some(AMQPType::ShortUInt)      => "be_u16",
    Some(AMQPType::LongInt)        => "be_i32",
    Some(AMQPType::LongUInt)       => "be_u32",
    Some(AMQPType::LongLongInt)    => "be_i64",
    Some(AMQPType::LongLongUInt)   => "be_u64",
    Some(AMQPType::ShortString)    => "map!(short_string, |s:&str| s.to_string())",
    Some(AMQPType::LongString)     => "map!(long_string,  |s:&str| s.to_string())",
    Some(AMQPType::FieldTable)     => "field_table",
    Some(AMQPType::Timestamp)      => "be_u64",
    Some(_)                        => unimplemented!(),
    None                           => {
      let data:  serde_json::Map<String, serde_json::Value> = serde_json::from_value(rc.context().data().clone()).unwrap();
      let specs: serde_json::Map<String, serde_json::Value> =  serde_json::from_value(data.get("specs").unwrap().clone()).unwrap();
      let domains: Vec<AMQPDomain> =  serde_json::from_value(specs.get("domains").unwrap().clone()).unwrap();

      let lookup_domain = arg.domain.clone().unwrap();
      domains.iter().find(|d| d.name == lookup_domain).map(|d| match d.amqp_type {
        AMQPType::Boolean        => "map!(be_u8, |u| u != 0)",
        AMQPType::ShortShortInt  => "be_i8",
        AMQPType::ShortShortUInt => "be_u8",
        AMQPType::ShortInt       => "be_i16",
        AMQPType::ShortUInt      => "be_u16",
        AMQPType::LongInt        => "be_i32",
        AMQPType::LongUInt       => "be_u32",
        AMQPType::LongLongInt    => "be_i64",
        AMQPType::LongLongUInt   => "be_u64",
        AMQPType::ShortString    => "map!(short_string, |s:&str| s.to_string())",
        AMQPType::LongString     => "map!(long_string,  |s:&str| s.to_string())",
        AMQPType::FieldTable     => "field_table",
        AMQPType::Timestamp      => "be_u64",
        _                        => unimplemented!(),
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

  let specs = get_specs(rc);
  let (arg_type, arg_name) = get_type_and_name(&arg, &specs);

  let rendered = match arg_type {
    AMQPType::Boolean        => "gen_bool",
    AMQPType::ShortShortInt  => "gen_be_i8",
    AMQPType::ShortShortUInt => "gen_be_u8",
    AMQPType::ShortInt       => "gen_be_i16",
    AMQPType::ShortUInt      => "gen_be_u16",
    AMQPType::LongInt        => "gen_be_i32",
    AMQPType::LongUInt       => "gen_be_u32",
    AMQPType::LongLongInt    => "gen_be_i64",
    AMQPType::LongLongUInt   => "gen_be_u64",
    AMQPType::ShortString    => "gen_short_string",
    AMQPType::LongString     => "gen_long_string",
    AMQPType::FieldTable     => "gen_field_table",
    AMQPType::Timestamp      => "gen_be_u64",
    _                        => unimplemented!(),
  };
  try!(rc.writer.write(rendered.as_bytes()));

  Ok(())
}
