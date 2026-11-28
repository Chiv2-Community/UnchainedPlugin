use anyhow::Result;
use std::path::Path;

fn main() -> Result<()> {
    let output = std::env::args_os().nth(1).expect("Expected an output directory");

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();

    unchained_macros_native_docs_generator::generate(&workspace.join("unchained-plugin"), workspace, Path::new(&output))
}
