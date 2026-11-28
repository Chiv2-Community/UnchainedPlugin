use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() {
            collect(&path, files)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

pub fn run() {
    run_for_package().expect("failed to format package");
}

fn run_for_package() -> Result<()> {
    let mut files = Vec::new();
    // Cargo build script: only write sources in the configuring checkout.
    let dir = env::current_dir()?;
    println!("cargo:rerun-if-env-changed=UNCHAINED_FORMAT_ROOT");
    let Some(root) = env::var_os("UNCHAINED_FORMAT_ROOT") else {
        return Ok(());
    };
    if !dir.starts_with(Path::new(&root)) {
        return Ok(());
    }
    let config = dir
        .ancestors()
        .map(|p| p.join("rustfmt.toml"))
        .find(|p| p.is_file())
        .ok_or("missing macro crate rustfmt.toml")?;
    println!("cargo:rerun-if-changed={}", config.display());
    for name in ["src", "tests", "examples", "benches"] {
        let path = dir.join(name);
        if path.is_dir() {
            println!("cargo:rerun-if-changed={name}");
            collect(&path, &mut files)?;
        }
    }
    format_files(files, false)
}

pub fn format_files(files: Vec<PathBuf>, check: bool) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    let mut command = Command::new("rustfmt");
    command.args(["--edition", "2021", "--config", "skip_children=true"]);
    if check {
        command.arg("--check");
    }
    if !command.args(files).status()?.success() {
        return Err("rustfmt failed; run cargo fmt-macros".into());
    }
    Ok(())
}
