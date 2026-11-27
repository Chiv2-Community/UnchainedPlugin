use anyhow::Result;
use std::path::Path;

fn main() -> Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .expect("Expected an output directory");
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    unchained_native_docs::generate(manifest, manifest.parent().unwrap(), Path::new(&output))
}
