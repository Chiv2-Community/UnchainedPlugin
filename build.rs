#[path = "build/provenance.rs"]
mod provenance;

fn main() {
    println!("cargo:rerun-if-changed=build/provenance.rs");
    println!("cargo:rerun-if-changed=resources/plugin.rc");
    let (commit, dirty) = provenance::generate();
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        compile_resources(&commit, &dirty);
    }
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
