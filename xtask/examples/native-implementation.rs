use anyhow::Result;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use unchained_native_docs::cpp::{find_class, find_implementation};

#[derive(Clone, Copy, ValueEnum)]
enum LookupKind {
    Implementation,
    Class,
}

#[derive(Parser)]
struct Args {
    #[arg(long)]
    root: PathBuf,
    #[arg(long)]
    header: PathBuf,
    #[arg(long)]
    target: String,
    #[arg(long, value_enum, default_value = "implementation")]
    kind: LookupKind,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let result = match args.kind {
        LookupKind::Implementation => find_implementation(&args.root, &args.header, &args.target)?,
        LookupKind::Class => find_class(&args.root, &args.header, &args.target)?,
    };
    let output = result.map(|implementation| {
        serde_json::json!({
            "path": implementation.path, "line": implementation.line, "code": implementation.code,
        })
    });
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
