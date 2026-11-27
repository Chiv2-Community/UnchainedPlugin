use std::{env, path::Path};

pub fn run() {
    println!("cargo:rerun-if-changed=build/generate_native_binding_docs.rs");
    unchained_native_docs::generate(
        Path::new(&env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        Path::new(&env::var_os("OUT_DIR").unwrap()),
    )
    .expect("Failed to generate native documentation");
}
