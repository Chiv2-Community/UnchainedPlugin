use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .args(args)
        .current_dir(std::env::var_os("CARGO_MANIFEST_DIR")?)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn provenance() -> (String, String) {
    let Some(commit) = git(&["rev-parse", "--verify", "HEAD"]) else {
        return ("unknown".into(), "unknown".into());
    };

    // Track source edits and worktree-specific Git state without watching build outputs.
    if let Some(files) = git(&["ls-files", "-z"]) {
        for file in files.split('\0').filter(|file| !file.is_empty()) {
            println!("cargo:rerun-if-changed={file}");
        }
    }
    for name in ["HEAD", "index", "refs", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            let path = path.trim();
            if std::path::Path::new(path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }

    // Match git describe --dirty: untracked files do not mark a build dirty.
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .map(|status| (!status.is_empty()).to_string())
        .unwrap_or_else(|| "unknown".into());
    (commit.trim().into(), dirty)
}

fn main() {
    println!("cargo:rerun-if-changed=resources/plugin.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }

    let [major, minor, patch] = [
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    ]
    .map(|component| {
        component
            .parse::<u16>()
            .expect("Windows version components must be integers from 0 to 65535")
    });
    let (commit, dirty) = provenance();
    let definitions = [
        format!("PLUGIN_VERSION_NUM={major},{minor},{patch},0"),
        format!("PLUGIN_VERSION=\"{}\\0\"", env!("CARGO_PKG_VERSION")),
        format!("PLUGIN_GIT_COMMIT=\"{commit}\\0\""),
        format!("PLUGIN_GIT_DIRTY=\"{dirty}\\0\""),
    ];

    embed_resource::compile_for_cdylib("resources/plugin.rc", &definitions)
        .manifest_required()
        .expect("Failed to compile DLL version resources");
}
