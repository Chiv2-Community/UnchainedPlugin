//! Select build modules at compile time, so each package needs only its own build dependencies.
use serde::Deserialize;
use std::{env, fs, path::Path};

#[derive(Deserialize)]
struct Manifest {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    #[serde(default)]
    metadata: Metadata,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Metadata {
    #[serde(default)]
    pre_build_steps: Vec<PreBuildStep>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PreBuildStep {
    GenerateNativeBindingDocs,
    EmbedBuildMetadata,
    Format,
}

#[proc_macro]
pub fn build_script(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path = Path::new(&env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml");
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let manifest: Manifest = toml::from_str(&source).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut modules = String::new();
    let mut calls = String::new();
    for step in manifest.package.metadata.pre_build_steps {
        let (name, path) = match step {
            PreBuildStep::GenerateNativeBindingDocs => ("generate_native_binding_docs", "../unchained-plugin/build/generate_native_binding_docs.rs"),
            PreBuildStep::EmbedBuildMetadata => ("embed_build_metadata", "../unchained-plugin/build/embed_build_metadata.rs"),
            PreBuildStep::Format => ("format", "format.rs"),
        };
        modules.push_str(&format!("#[path = \"{path}\"] mod {name};"));
        calls.push_str(&format!("{name}::run();"));
    }
    // Recompile the build script when the task list changes, even if Cargo
    // considers the metadata-only manifest edit irrelevant to compilation.
    format!(
        r#"
        const _: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        {modules}
        fn main() {{ println!("cargo:rerun-if-changed=Cargo.toml"); {calls} }}
    "#
    )
    .parse()
    .unwrap()
}
