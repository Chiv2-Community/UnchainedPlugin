//! Add blank lines after completed multiline brace groups. Run after rustfmt.
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn closing_lines(tokens: TokenStream, lines: &mut BTreeSet<usize>) {
    for token in tokens {
        if let TokenTree::Group(group) = token {
            let end = group.span_close().end().line;
            if group.delimiter() == Delimiter::Brace && group.span_open().start().line < end {
                lines.insert(end);
            }
            closing_lines(group.stream(), lines);
        }
    }
}

fn spaced(source: &str) -> Result<String> {
    let tokens = source.parse()?;
    let mut ends = BTreeSet::new();
    closing_lines(tokens, &mut ends);
    let mut lines = source.split_inclusive('\n').peekable();
    let mut out = String::with_capacity(source.len());
    let mut n = 0;
    while let Some(line) = lines.next() {
        n += 1;
        out.push_str(line);
        if !ends.contains(&n) {
            continue;
        }
        let code = line.trim_start();
        // Allow trailing delimiters and a line comment on the closing line.
        let tail = code.trim_start_matches(['}', ')', ']', ';', ',']).trim();
        if !code.starts_with('}') || !(tail.is_empty() || tail.starts_with("//")) {
            continue;
        }
        if let Some(next) = lines.peek() {
            let next = next.trim();
            if !next.is_empty() && !next.starts_with(['{', '}', ')', ']', '.', '?', ',', ';']) {
                out.push_str(if line.ends_with("\r\n") { "\r\n" } else { "\n" });
            }
        }
    }
    Ok(out)
}

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

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1).peekable();
    let check = args.next_if(|arg| arg == "--check").is_some();
    let mut files: Vec<PathBuf> = args.map(PathBuf::from).collect();
    if files.is_empty() && env!("CARGO_PKG_NAME") == "xtask" {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let output = Command::new("git")
            .current_dir(root)
            .args([
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
                "--",
                "unchained-macros-*/*.rs",
            ])
            .output()?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
        }
        files.extend(
            std::str::from_utf8(&output.stdout)?
                .split_terminator('\0')
                .map(|path| root.join(path)),
        );
    } else if files.is_empty() {
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
    }
    let mut stale = false;
    for path in files {
        let source = fs::read_to_string(&path)?;
        let output = Command::new("rustfmt")
            .args([
                "--edition",
                "2021",
                "--config",
                "skip_children=true",
                "--emit",
                "stdout",
                "--quiet",
            ])
            .arg(&path)
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "{}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let formatted = spaced(std::str::from_utf8(&output.stdout)?)?;
        if formatted != source {
            if check {
                eprintln!("Formatting required: {}", path.display());
                stale = true;
            } else {
                fs::write(path, formatted)?;
            }
        }
    }
    if stale {
        return Err("run cargo fmt-macros".into());
    }
    Ok(())
}
