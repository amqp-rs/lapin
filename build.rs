extern crate amq_protocol_codegen;
extern crate amq_protocol_types;
extern crate handlebars;
extern crate serde;
#[macro_use] extern crate serde_derive;
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
    handlebars.register_helper("map_parser", Box::new(map_parser_helper));
    handlebars.register_helper("map_assign", Box::new(map_assignment_helper));
    handlebars.register_helper("map_generator", Box::new(map_generator_helper));

    handlebars.register_template_string("full", full_tpl).expect("Failed to register full template");
    handlebars.register_template_string("api", api_tpl).expect("Failed to register api template");

    let specs     = AMQProtocolDefinition::load();
    data.insert("specs".to_string(), specs);
    writeln!(f, "{}", handlebars.render("full", &data).expect("Failed to render full template"));
    writeln!(f2, "{}", handlebars.render("api", &data).expect("Failed to render full template"));

    //writeln!(f, "{}", specs.codegen_full(&full_tpl)).expect("Failed to generate generated.rs");
}

fn get_specs(rc: &mut RenderContext) ->  serde_json::Map<String, serde_json::Value> {
  let data:  serde_json::Map<String, serde_json::Value> = serde_json::from_value(rc.context().data().clone()).unwrap();
  let specs: serde_json::Map<String, serde_json::Value> = serde_json::from_value(data.get("specs").unwrap().clone()).unwrap();

  specs
}

fn get_arguments(h: &Helper, rc: &mut RenderContext, class_id_index: usize, method_id_index: usize) -> Vec<AMQPArgument> {
  let class_id:  u16 = serde_json::from_value(h.param(class_id_index).unwrap().value().clone()).unwrap();
  let method_id: u16 = serde_json::from_value(h.param(method_id_index).unwrap().value().clone()).unwrap();

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
    AMQPType::Boolean        => {
      let arguments = get_arguments(h, rc, 1, 2);
      println!("arguments:\n{:?}", arguments);

      //if it's the first of a list of booleans, write a bits!( ... ), otherwise do nothing
      let position = arguments.iter().position(|a| a.name == arg.name).unwrap();
      let should_parse_bits = {
        position == 0 ||
        arguments[position-1].amqp_type != AMQPType::Boolean
      };

      if !should_parse_bits {
        "".to_string()
      } else {
        let mut bit_args: Vec<AMQPArgument> = arguments.iter().skip(position).take_while(|a| a.amqp_type == AMQPType::Boolean).cloned().collect();
        bit_args.reverse();
        //FIXME: assuming there are less than 8 elements
        let offset = 8 - bit_args.len();

        let mut handlebars = Handlebars::new();
        let mut data = BTreeMap::new();
        let mut bits_tpl = String::new();
        std::fs::File::open("templates/bits_parse.rs").expect("Failed to open bits template").read_to_string(&mut bits_tpl).expect("Failed to read bits template");

        handlebars.register_escape_fn(handlebars::no_escape);
        handlebars.register_helper("snake", Box::new(snake_helper));
        handlebars.register_template_string("bits", bits_tpl).expect("Failed to register bits template");

        let args: serde_json::Value =  serde_json::to_value(bit_args).unwrap();
        data.insert("arguments".to_string(), args);
        data.insert("offset".to_string(), serde_json::to_value(offset).unwrap());

        handlebars.render("bits", &data).unwrap()
      }
    },
    AMQPType::ShortShortInt  => format!("{} : {} >>", snake_case(&arg.name), "be_i8"),
    AMQPType::ShortShortUInt => format!("{} : {} >>", snake_case(&arg.name), "be_u8"),
    AMQPType::ShortInt       => format!("{} : {} >>", snake_case(&arg.name),"be_i16"),
    AMQPType::ShortUInt      => format!("{} : {} >>", snake_case(&arg.name),"be_u16"),
    AMQPType::LongInt        => format!("{} : {} >>", snake_case(&arg.name),"be_i32"),
    AMQPType::LongUInt       => format!("{} : {} >>", snake_case(&arg.name),"be_u32"),
    AMQPType::LongLongInt    => format!("{} : {} >>", snake_case(&arg.name),"be_i64"),
    AMQPType::LongLongUInt   => format!("{} : {} >>", snake_case(&arg.name),"be_u64"),
    AMQPType::ShortString    => format!("{} : {} >>", snake_case(&arg.name),"map!(short_string, |s:&str| s.to_string())"),
    AMQPType::LongString     => format!("{} : {} >>", snake_case(&arg.name),"map!(long_string,  |s:&str| s.to_string())"),
    AMQPType::FieldTable     => format!("{} : {} >>", snake_case(&arg.name),"field_table"),
    AMQPType::Timestamp      => format!("{} : {} >>", snake_case(&arg.name),"be_u64"),
    _                        => unimplemented!(),
  };
  try!(rc.writer.write(&rendered.as_bytes()));

  Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NameId {
  pub name: String,
  pub id:   usize,
}

fn map_assignment_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val).unwrap();

  let rendered = match arg.amqp_type {
    AMQPType::Boolean        => {
      let arguments = get_arguments(h, rc, 1, 2);
      println!("arguments:\n{:?}", arguments);

      //if it's the first of a list of booleans, write a bits!( ... ), otherwise do nothing
      let position = arguments.iter().position(|a| a.name == arg.name).unwrap();
      let should_parse_bits = {
        position == 0 ||
        arguments[position-1].amqp_type != AMQPType::Boolean
      };

      if !should_parse_bits {
        "".to_string()
      } else {
        let mut bit_args: Vec<AMQPArgument> = arguments.iter().skip(position).take_while(|a| a.amqp_type == AMQPType::Boolean).cloned().collect();
        bit_args.reverse();
        let bit_args:Vec<NameId> = bit_args.iter().enumerate().map(|(i,val)| NameId { name: val.name.clone(), id: i }).collect();

        let mut handlebars = Handlebars::new();
        let mut data = BTreeMap::new();
        let mut bits_tpl = String::new();
        std::fs::File::open("templates/bits_assign.rs").expect("Failed to open bits template").read_to_string(&mut bits_tpl).expect("Failed to read bits template");

        handlebars.register_escape_fn(handlebars::no_escape);
        handlebars.register_helper("snake", Box::new(snake_helper));
        handlebars.register_template_string("bits", bits_tpl).expect("Failed to register bits template");

        let args: serde_json::Value =  serde_json::to_value(bit_args).unwrap();
        data.insert("arguments".to_string(), args);

        handlebars.render("bits", &data).unwrap()
      }
    },
    _ => format!("{}: {},", snake_case(&arg.name), snake_case(&arg.name)),
  };
  try!(rc.writer.write(&rendered.as_bytes()));

  Ok(())
}

fn map_generator_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  println!("val1: {:?}", h.param(0));
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val).unwrap();

  let rendered = match arg.amqp_type {
    AMQPType::Boolean        => {
      let arguments = get_arguments(h, rc, 1, 2);
      println!("arguments:\n{:?}", arguments);

      //if it's the first of a list of booleans, write a bits!( ... ), otherwise do nothing
      let position = arguments.iter().position(|a| a.name == arg.name).unwrap();
      let should_parse_bits = {
        position == 0 ||
        arguments[position-1].amqp_type != AMQPType::Boolean
      };

      if !should_parse_bits {
        "".to_string()
      } else {
        let mut bit_args: Vec<AMQPArgument> = arguments.iter().skip(position).take_while(|a| a.amqp_type == AMQPType::Boolean).cloned().collect();

        let mut handlebars = Handlebars::new();
        let mut data = BTreeMap::new();
        let mut bits_tpl = String::new();
        std::fs::File::open("templates/bits_gen.rs").expect("Failed to open bits template").read_to_string(&mut bits_tpl).expect("Failed to read bits template");

        handlebars.register_escape_fn(handlebars::no_escape);
        handlebars.register_helper("snake", Box::new(snake_helper));
        handlebars.register_template_string("bits", bits_tpl).expect("Failed to register bits template");

        let args: serde_json::Value =  serde_json::to_value(bit_args).unwrap();
        data.insert("arguments".to_string(), args);

        handlebars.render("bits", &data).unwrap()
      }
    },
    AMQPType::ShortShortInt  => format!(">> {}(&method.{})", "gen_be_i8", snake_case(&arg.name)),
    AMQPType::ShortShortUInt => format!(">> {}(&method.{})", "gen_be_u8", snake_case(&arg.name)),
    AMQPType::ShortInt       => format!(">> {}(&method.{})", "gen_be_i16", snake_case(&arg.name)),
    AMQPType::ShortUInt      => format!(">> {}(&method.{})", "gen_be_u16", snake_case(&arg.name)),
    AMQPType::LongInt        => format!(">> {}(&method.{})", "gen_be_i32", snake_case(&arg.name)),
    AMQPType::LongUInt       => format!(">> {}(&method.{})", "gen_be_u32", snake_case(&arg.name)),
    AMQPType::LongLongInt    => format!(">> {}(&method.{})", "gen_be_i64", snake_case(&arg.name)),
    AMQPType::LongLongUInt   => format!(">> {}(&method.{})", "gen_be_u64", snake_case(&arg.name)),
    AMQPType::ShortString    => format!(">> {}(&method.{})", "gen_short_string", snake_case(&arg.name)),
    AMQPType::LongString     => format!(">> {}(&method.{})", "gen_long_string", snake_case(&arg.name)),
    AMQPType::FieldTable     => format!(">> {}(&method.{})", "gen_field_table", snake_case(&arg.name)),
    AMQPType::Timestamp      => format!(">> {}(&method.{})", "gen_be_u64", snake_case(&arg.name)),
    _                        => unimplemented!(),
  };
  try!(rc.writer.write(rendered.as_bytes()));

  Ok(())
}
