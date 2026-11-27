//! Select build modules at compile time, so each package needs only its own build dependencies.
use std::{env, fs, path::Path};

#[proc_macro]
pub fn build_script(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let manifest = Path::new(&env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml");
    let manifest: toml::Table = fs::read_to_string(manifest).unwrap().parse().unwrap();
    let mut modules = String::new();
    let mut calls = String::new();
    if let Some(steps) = manifest["package"].get("metadata").and_then(|m| m.get("pre-build-steps")) {
        for step in steps.as_array().expect("package.metadata.pre-build-steps must be an array") {
            let (name, path) = match step.as_str() {
                Some("plugin") => ("plugin", "build/mod.rs"),
                Some("format") => ("format", "build/format.rs"),
                _ => panic!("unknown pre-build step: {step}"),
            };
            modules.push_str(&format!("#[path = \"{path}\"] mod {name};"));
            calls.push_str(&format!("{name}::run();"));
        }
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
