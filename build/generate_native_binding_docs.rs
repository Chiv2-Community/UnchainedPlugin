use std::{env, path::Path};

pub fn run() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let plugin = Path::new(&manifest);
    unchained_macros_native_docs_generator::generate(
        plugin,
        plugin.parent().unwrap(),
        Path::new(&env::var_os("OUT_DIR").unwrap()),
    )
    .expect("Failed to generate native documentation");
}
