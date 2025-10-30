use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use uniffi_bindgen::bindings::SwiftBindingGenerator;

fn main() -> Result<()> {
    generate_swift_bindings()
}

fn generate_swift_bindings() -> Result<()> {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .context("failed to locate repository root")?;

    let udl_path = manifest_dir.join("src/prism_core.udl");
    let output_dir = repo_root.join("swift/PrismFFI/Sources/PrismFFI");

    std::fs::create_dir_all(output_dir.as_std_path())
        .with_context(|| format!("failed to create {}", output_dir))?;

    uniffi_bindgen::generate_bindings(
        udl_path.as_path(),
        None,
        SwiftBindingGenerator,
        Some(output_dir.as_path()),
        None,
        Some("prism_core"),
        true,
    )
    .with_context(|| format!("failed to generate Swift bindings from {}", udl_path))?;

    println!("Generated Swift bindings in {}", output_dir);

    Ok(())
}
