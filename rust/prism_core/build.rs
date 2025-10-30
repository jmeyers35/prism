fn main() {
    let udl_path = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("src/prism_core.udl");
    let udl_path = udl_path.to_str().expect("UDL path contains invalid UTF-8");
    uniffi_build::generate_scaffolding_for_crate(udl_path, "prism_core")
        .expect("failed to generate UniFFI scaffolding");
}
