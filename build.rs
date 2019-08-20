use amq_protocol_codegen::{CodeGenerator, HandlebarsAMQPExtension};
use serde_json::{from_str, Value};

use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let template = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/channel.rs"));
    let extra = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/templates/async-extra.json"
    ));
    let data = from_str::<Value>(extra).expect("Failed to parse extra file");

    CodeGenerator::simple_codegen_with_data(
        &out_dir,
        "channel",
        "channel",
        template,
        "protocol",
        Some(data),
    );
}
