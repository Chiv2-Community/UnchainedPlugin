use git2::{Delta, DescribeOptions, Oid, Repository, StatusOptions, Statuses};

#[derive(Default)]
struct Provenance {
    commit: Option<Oid>,
    tag: Option<String>,
    dirty: Option<bool>,
    changes: Vec<String>,
}

fn provenance() -> Option<Provenance> {
    let repo = Repository::discover(std::env::var_os("CARGO_MANIFEST_DIR")?).ok()?;
    let commit = repo.head().ok()?.peel_to_commit().ok()?.id();
    track_repository(&repo);

    let statuses = repo
        .statuses(Some(
            StatusOptions::new()
                .include_untracked(true)
                .recurse_untracked_dirs(true)
                .renames_head_to_index(true)
                .renames_index_to_workdir(true),
        ))
        .ok();
    // Zero candidates restricts describe to tags pointing at HEAD.
    let tag = repo
        .describe(
            DescribeOptions::new()
                .describe_tags()
                .max_candidates_tags(0),
        )
        .and_then(|description| description.format(None))
        .ok();
    Some(Provenance {
        commit: Some(commit),
        tag,
        dirty: statuses.as_ref().map(|statuses| !statuses.is_empty()),
        changes: statuses
            .as_ref()
            .map(uncommitted_changes)
            .unwrap_or_default(),
    })
}

fn uncommitted_changes(statuses: &Statuses<'_>) -> Vec<String> {
    let mut changes = Vec::new();
    for entry in statuses.iter() {
        if entry.status().is_conflicted() {
            changes.push(format!(
                "Conflicted: {}",
                String::from_utf8_lossy(entry.path_bytes()).escape_debug()
            ));
            continue;
        }
        for (area, delta) in [
            ("Staged", entry.head_to_index()),
            ("Unstaged", entry.index_to_workdir()),
        ] {
            let Some(delta) = delta else { continue };
            let path = delta
                .new_file()
                .path_bytes()
                .or_else(|| delta.old_file().path_bytes())
                .unwrap_or(entry.path_bytes());
            let status = if delta.status() == Delta::Untracked {
                "Untracked".to_owned()
            } else {
                format!("{area} {:?}", delta.status())
            };
            changes.push(format!(
                "{status}: {}",
                String::from_utf8_lossy(path).escape_debug()
            ));
        }
    }
    changes
}

// Watch tracked sources and worktree-aware Git state, not build outputs.
fn track_repository(repo: &Repository) {
    if let (Some(workdir), Ok(index)) = (repo.workdir(), repo.index()) {
        for entry in index.iter() {
            let file = workdir.join(String::from_utf8_lossy(&entry.path).as_ref());
            println!("cargo:rerun-if-changed={}", file.display());
        }
    }
    for path in [
        repo.path().join("HEAD"),
        repo.path().join("index"),
        repo.commondir().join("refs"),
        repo.commondir().join("packed-refs"),
    ] {
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=resources/plugin.rc");
    let provenance = provenance().unwrap_or_default();
    let commit = provenance
        .commit
        .map(|commit| commit.to_string())
        .unwrap_or_else(|| "unknown".into());
    let dirty = provenance
        .dirty
        .map(|dirty| dirty.to_string())
        .unwrap_or_else(|| "unknown".into());
    write_provenance(&provenance, &commit);
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        compile_resources(&commit, &dirty);
    }
}

fn write_provenance(provenance: &Provenance, commit: &str) {
    let revision = match &provenance.tag {
        Some(tag) => format!("Revision: {tag} ({commit})"),
        None => format!("Revision: {commit}"),
    };
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let source = format!(
        concat!(
            "pub(super) const COMMIT: &str = {commit:?};\n",
            "pub(super) const REVISION: &str = {revision:?};\n",
            "pub(super) const DIRTY: bool = {dirty};\n",
            "pub(super) const UNCOMMITTED_CHANGES: &[&str] = &{changes:?};\n",
        ),
        commit = commit,
        revision = revision,
        dirty = provenance.dirty == Some(true),
        changes = provenance.changes,
    );
    std::fs::write(output.join("provenance.rs"), source).expect("Failed to write build provenance");
}

fn compile_resources(commit: &str, dirty: &str) {
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
