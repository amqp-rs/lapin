fn main() {
    println!("cargo::rerun-if-env-changed=LAPIN_CODEGEN_DIR");
    println!("cargo::rerun-if-changed=templates/channel.rs");
    println!("cargo::rerun-if-changed=templates/lapin.json");
    println!("cargo::rerun-if-changed=templates/includes");

    #[cfg(feature = "codegen-internal")]
    codegen::generate()
}

#[cfg(feature = "codegen-internal")]
mod codegen {
    use amq_protocol_codegen::{CodeGenerator, HandlebarsAMQPExtension};
    use serde_json::Value;

    pub(super) fn generate() {
        let cargo_out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is not defined");
        let out_dir = std::env::var("LAPIN_CODEGEN_DIR").unwrap_or_else(|_| cargo_out_dir.clone());
        let extra = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/lapin.json"));
        let data = serde_json::from_str::<Value>(extra).expect("Failed to parse extra file");

        std::fs::create_dir_all(&out_dir).expect("failed to create codegen directory");
        codegen(&out_dir, data, "channel");
        // The codegen feature includes the copy in OUT_DIR
        #[cfg(feature = "codegen")]
        if out_dir != cargo_out_dir {
            let generated = std::path::Path::new(&out_dir).join("channel.rs");
            let cargo_generated = std::path::Path::new(&cargo_out_dir).join("channel.rs");
            // Read before writing: both paths may name the same file
            let code = std::fs::read(&generated)
                .unwrap_or_else(|err| panic!("Failed to read {generated:?}: {err}"));
            std::fs::write(&cargo_generated, code)
                .unwrap_or_else(|err| panic!("Failed to write {cargo_generated:?}: {err}"));
        }
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
