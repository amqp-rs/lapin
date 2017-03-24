extern crate amq_protocol;

use amq_protocol::codegen::*;

use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

fn main() {
    let out_dir        = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let dest_path      = Path::new(&out_dir).join("api.rs");
    let mut f          = File::create(&dest_path).expect("Failed to create api.rs");
    let mut api_tpl    = String::new();
    let mut handlebars = CodeGenerator::new().register_amqp_helpers();
    let mut data       = BTreeMap::new();
    let specs          = AMQProtocolDefinition::load();

    std::fs::File::open("templates/api.rs").expect("Failed to open main template").read_to_string(&mut api_tpl).expect("Failed to read main template");
    handlebars.register_template_string("api", api_tpl).expect("Failed to register api template");
    data.insert("specs".to_string(), specs);

    writeln!(f, "{}", handlebars.render("api", &data).expect("Failed to render api template")).expect("Failed to write api.rs");
}
