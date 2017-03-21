extern crate amq_protocol_codegen;
extern crate amq_protocol_types;
extern crate handlebars;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;

use amq_protocol_codegen::*;
use amq_protocol_types::*;
use amq_protocol_types::types::*;
use handlebars::{Handlebars,Helper,RenderContext,RenderError};

use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

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

    let mut handlebars = CodeGenerator::new().register_amqp_helpers();
    let mut data = BTreeMap::new();

    handlebars.register_helper("map_parser", Box::new(map_parser_helper));
    handlebars.register_helper("map_assign", Box::new(map_assignment_helper));
    handlebars.register_helper("map_generator", Box::new(map_generator_helper));

    handlebars.register_template_string("full", full_tpl).expect("Failed to register full template");
    handlebars.register_template_string("api", api_tpl).expect("Failed to register api template");

    let specs     = AMQProtocolDefinition::load();
    data.insert("specs".to_string(), specs);
    writeln!(f, "{}", handlebars.render("full", &data).expect("Failed to render full template")).expect("Failed to write generated.rs");
    writeln!(f2, "{}", handlebars.render("api", &data).expect("Failed to render api template")).expect("Failed to write api.rs");
}

fn map_parser_helper(h: &Helper, _: &Handlebars, rc: &mut RenderContext) -> Result<(), RenderError> {
  println!("val1: {:?}", h.param(0));
  let val = h.param(0).unwrap().value().clone();
  let arg:AMQPArgument = serde_json::from_value(val).unwrap();

  let rendered = match arg.amqp_type {
    AMQPType::Boolean        => {
      let val = h.param(1).unwrap().value().clone();
      let arguments: Vec<AMQPArgument> = serde_json::from_value(val).unwrap();
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

        let mut handlebars = CodeGenerator::new().register_amqp_helpers();
        let mut data = BTreeMap::new();
        let mut bits_tpl = String::new();
        std::fs::File::open("templates/bits_parse.rs").expect("Failed to open bits template").read_to_string(&mut bits_tpl).expect("Failed to read bits template");

        handlebars.register_template_string("bits", bits_tpl).expect("Failed to register bits template");

        let args: serde_json::Value =  serde_json::to_value(bit_args).unwrap();
        data.insert("arguments".to_string(), args);
        data.insert("offset".to_string(), serde_json::to_value(offset).unwrap());

        handlebars.render("bits", &data).unwrap()
      }
    },
    AMQPType::ShortShortInt  => format!("{} : {} >>", snake_case(&arg.name), "parse_short_short_int"),
    AMQPType::ShortShortUInt => format!("{} : {} >>", snake_case(&arg.name), "parse_short_short_uint"),
    AMQPType::ShortInt       => format!("{} : {} >>", snake_case(&arg.name), "parse_short_int"),
    AMQPType::ShortUInt      => format!("{} : {} >>", snake_case(&arg.name), "parse_short_uint"),
    AMQPType::LongInt        => format!("{} : {} >>", snake_case(&arg.name), "parse_long_int"),
    AMQPType::LongUInt       => format!("{} : {} >>", snake_case(&arg.name), "parse_long_uint"),
    AMQPType::LongLongInt    => format!("{} : {} >>", snake_case(&arg.name), "parse_long_long_int"),
    AMQPType::LongLongUInt   => format!("{} : {} >>", snake_case(&arg.name), "parse_long_long_uint"),
    AMQPType::ShortString    => format!("{} : {} >>", snake_case(&arg.name), "parse_short_string"),
    AMQPType::LongString     => format!("{} : {} >>", snake_case(&arg.name), "parse_long_string"),
    AMQPType::Float          => format!("{} : {} >>", snake_case(&arg.name), "parse_float"),
    AMQPType::Double         => format!("{} : {} >>", snake_case(&arg.name), "parse_double"),
    AMQPType::DecimalValue   => format!("{} : {} >>", snake_case(&arg.name), "parse_decimal_value"),
    AMQPType::FieldTable     => format!("{} : {} >>", snake_case(&arg.name), "parse_field_table"),
    AMQPType::Timestamp      => format!("{} : {} >>", snake_case(&arg.name), "parse_timestamp"),
    AMQPType::FieldArray     => format!("{} : {} >>", snake_case(&arg.name), "parse_field_array"),
    AMQPType::Void           => "".to_string(),
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
      let val = h.param(1).unwrap().value().clone();
      let arguments: Vec<AMQPArgument> = serde_json::from_value(val).unwrap();
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

        let mut handlebars = CodeGenerator::new().register_amqp_helpers();
        let mut data = BTreeMap::new();
        let mut bits_tpl = String::new();
        std::fs::File::open("templates/bits_assign.rs").expect("Failed to open bits template").read_to_string(&mut bits_tpl).expect("Failed to read bits template");

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
      let val = h.param(1).unwrap().value().clone();
      let arguments: Vec<AMQPArgument> = serde_json::from_value(val).unwrap();
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
        let bit_args: Vec<AMQPArgument> = arguments.iter().skip(position).take_while(|a| a.amqp_type == AMQPType::Boolean).cloned().collect();

        let mut handlebars = CodeGenerator::new().register_amqp_helpers();
        let mut data = BTreeMap::new();
        let mut bits_tpl = String::new();
        std::fs::File::open("templates/bits_gen.rs").expect("Failed to open bits template").read_to_string(&mut bits_tpl).expect("Failed to read bits template");

        handlebars.register_template_string("bits", bits_tpl).expect("Failed to register bits template");

        let args: serde_json::Value =  serde_json::to_value(bit_args).unwrap();
        data.insert("arguments".to_string(), args);

        handlebars.render("bits", &data).unwrap()
      }
    },
    AMQPType::ShortShortInt  => format!(">> {}(&method.{})", "gen_short_short_int", snake_case(&arg.name)),
    AMQPType::ShortShortUInt => format!(">> {}(&method.{})", "gen_short_short_uint", snake_case(&arg.name)),
    AMQPType::ShortInt       => format!(">> {}(&method.{})", "gen_short_int", snake_case(&arg.name)),
    AMQPType::ShortUInt      => format!(">> {}(&method.{})", "gen_short_uint", snake_case(&arg.name)),
    AMQPType::LongInt        => format!(">> {}(&method.{})", "gen_long_int", snake_case(&arg.name)),
    AMQPType::LongUInt       => format!(">> {}(&method.{})", "gen_long_uint", snake_case(&arg.name)),
    AMQPType::LongLongInt    => format!(">> {}(&method.{})", "gen_long_long_int", snake_case(&arg.name)),
    AMQPType::LongLongUInt   => format!(">> {}(&method.{})", "gen_long_long_uint", snake_case(&arg.name)),
    AMQPType::Float          => format!(">> {}(&method.{})", "gen_float", snake_case(&arg.name)),
    AMQPType::Double         => format!(">> {}(&method.{})", "gen_double", snake_case(&arg.name)),
    AMQPType::DecimalValue   => format!(">> {}(&method.{})", "gen_decimal_value", snake_case(&arg.name)),
    AMQPType::ShortString    => format!(">> {}(&method.{})", "gen_short_string", snake_case(&arg.name)),
    AMQPType::LongString     => format!(">> {}(&method.{})", "gen_long_string", snake_case(&arg.name)),
    AMQPType::FieldTable     => format!(">> {}(&method.{})", "gen_field_table", snake_case(&arg.name)),
    AMQPType::Timestamp      => format!(">> {}(&method.{})", "gen_timestamp", snake_case(&arg.name)),
    AMQPType::FieldArray     => format!(">> {}(&method.{})", "gen_field_array", snake_case(&arg.name)),
    AMQPType::Void           => "".to_string(),
  };
  try!(rc.writer.write(rendered.as_bytes()));

  Ok(())
}
