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
    pre_build_steps: Vec<String>,
}

#[proc_macro]
pub fn build_script(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path = Path::new(&env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml");
    let source = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let manifest: Manifest = toml::from_str(&source).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut modules = String::new();
    let mut calls = String::new();
    for step in manifest.package.metadata.pre_build_steps {
        assert!(
            step != "build" && step.split('-').all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())),
            "{}: expected a kebab-case pre-build step name (`build` is reserved), got {step:?}", path.display()
        );
        let name = step.replace('-', "_");
        modules.push_str(&format!("#[path = \"{name}.rs\"] mod step_{name};"));
        calls.push_str(&format!("step_{name}::run();"));
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
