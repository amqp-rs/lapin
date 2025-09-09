fn main() {
    println!("cargo::rerun-if-env-changed=LAPIN_CODEGEN_DIR");
    println!("cargo::rerun-if-env-changed=LAPIN_CODEGEN_FILE");
    println!("cargo::rerun-if-changed=templates/channel.rs");
    println!("cargo::rerun-if-changed=templates/lapin.json");

    #[cfg(feature = "codegen-internal")]
    codegen::generate()
}

#[cfg(feature = "codegen-internal")]
mod codegen {
    use amq_protocol_codegen::{CodeGenerator, HandlebarsAMQPExtension};
    use serde_json::Value;

    pub(super) fn generate() {
        let out_dir = std::env::var("LAPIN_CODEGEN_DIR")
            .or_else(|_| std::env::var("OUT_DIR"))
            .expect("OUT_DIR is not defined");
        let extra = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/lapin.json"));
        let data = serde_json::from_str::<Value>(extra).expect("Failed to parse extra file");

        std::fs::create_dir_all(&out_dir).expect("failed to create codegen directory");
        codegen(&out_dir, data, "channel");
    }

    fn codegen(out_dir: &str, data: Value, out_file: &str) {
        let template_path = format!("{0}/templates/{out_file}.rs", env!("CARGO_MANIFEST_DIR"));
        let template = std::fs::read_to_string(template_path).expect("failed to read template");

        CodeGenerator::simple_codegen_with_data(
            out_dir,
            out_file,
            out_file,
            &template,
            "protocol",
            Some(data),
        );
    }
}
