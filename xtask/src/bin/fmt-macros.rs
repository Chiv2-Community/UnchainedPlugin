#[allow(dead_code)]
#[path = "../../../build/format.rs"]
mod format;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).peekable();
    let check = args.next_if(|arg| arg == "--check").is_some();
    let mut files: Vec<std::path::PathBuf> = args.map(Into::into).collect();
    if files.is_empty() {
        let workspace = cargo_metadata::MetadataCommand::new().no_deps().exec()?;
        let root = &workspace.workspace_root;
        let mut git = std::process::Command::new("git");
        git.current_dir(root)
            .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--"]);
        let mut selected = vec![];
        for pkg in workspace.workspace_packages() {
            if pkg.metadata["pre-build-steps"]
                .as_array()
                .is_some_and(|s| s.iter().any(|v| v == "format"))
            {
                let dir = pkg.manifest_path.parent().unwrap().strip_prefix(root)?.as_str().replace('\\', "/");
                for sub in ["src", "tests", "examples", "benches"] {
                    selected.push(format!("{dir}/{sub}/*.rs"));
                }
            }
        }
        if selected.is_empty() {
            return Ok(());
        }
        let output = git.args(selected).output()?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
        }
        files.extend(
            std::str::from_utf8(&output.stdout)?
                .split_terminator('\0')
                .map(|p| root.join(p).into_std_path_buf()),
        );
    }
    format::format_files(files, check)
}
