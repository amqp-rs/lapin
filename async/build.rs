extern crate amq_protocol;

use amq_protocol::codegen::*;

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

    handlebars.register_template_string("full", full_tpl).expect("Failed to register full template");
    handlebars.register_template_string("api", api_tpl).expect("Failed to register api template");

    let specs     = AMQProtocolDefinition::load();
    data.insert("specs".to_string(), specs);
    writeln!(f, "{}", handlebars.render("full", &data).expect("Failed to render full template")).expect("Failed to write generated.rs");
    writeln!(f2, "{}", handlebars.render("api", &data).expect("Failed to render api template")).expect("Failed to write api.rs");
}
