fn main() {
    println!("cargo:rerun-if-env-changed=LAPIN_CODEGEN_DIR");
    println!("cargo:rerun-if-env-changed=LAPIN_CODEGEN_FILE");

    #[cfg(feature = "codegen-internal")]
    codegen()
}

#[cfg(feature = "codegen-internal")]
fn codegen() {
    use amq_protocol_codegen::{CodeGenerator, HandlebarsAMQPExtension};
    use serde_json::{Value, from_str};

    let out_dir = std::env::var("LAPIN_CODEGEN_DIR")
        .or_else(|_| std::env::var("OUT_DIR"))
        .expect("OUT_DIR is not defined");
    let out_file = std::env::var("LAPIN_CODEGEN_FILE").unwrap_or_else(|_| "channel".to_string());
    let template = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/channel.rs"));
    let extra = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/lapin.json"));
    let data = from_str::<Value>(extra).expect("Failed to parse extra file");

    CodeGenerator::simple_codegen_with_data(
        &out_dir,
        &out_file,
        "channel",
        template,
        "protocol",
        Some(data),
    );
}
