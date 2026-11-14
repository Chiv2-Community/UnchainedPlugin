//! Build the real backend Dockerfile from the exact SDK Git revision.
use std::{io::Read, path::Path, sync::OnceLock, time::Duration};
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::{SyncBuilder, SyncRunner},
    Container, GenericBuildableImage, GenericImage, ImageExt,
};

fn image() -> GenericImage {
    static IMAGE: OnceLock<GenericImage> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest: toml::Value = toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        let revision = manifest["workspace"]["dependencies"]["server-browser-client"]["rev"]
            .as_str().expect("SDK dependency must have an exact revision");
        assert_eq!(manifest["workspace"]["dependencies"]["server-browser-client-sdk"]["rev"].as_str(), Some(revision));
        assert!(revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let source = tempfile::tempdir().unwrap();
        let archive = reqwest::blocking::Client::builder().timeout(Duration::from_secs(90))
            .build().unwrap()
            .get(format!("https://codeload.github.com/Chiv2-Community/C2ServerBrowserBackend/tar.gz/{revision}"))
            .send().expect("download pinned backend source").error_for_status().unwrap();
        tar::Archive::new(flate2::read::GzDecoder::new(archive.take(32 * 1024 * 1024)))
            .unpack(source.path()).expect("unpack pinned backend source");
        let directory = source.path().join(format!("C2ServerBrowserBackend-{revision}"));
        GenericBuildableImage::new("unchained-backend-integration", revision)
            .with_dockerfile(directory.join("Dockerfile"))
            .with_file(&directory, ".")
            .build_image().expect("build authoritative backend Dockerfile")
    }).clone()
}

pub fn backend() -> (Container<GenericImage>, String) {
    // Signing keys and registry files exist only in this disposable container.
    let container = image()
        .with_entrypoint("/bin/sh")
        .with_exposed_port(8080.tcp())
        .with_wait_for(WaitFor::message_on_stderr("Uvicorn running on"))
        .with_env_var("CONFIG_DIR", "/tmp/browser-test")
        .with_env_var("ADMIN_KEY", "container-test-admin")
        .with_env_var("TRUSTED_PROXY_CIDRS", "")
        .with_env_var("DISCORD_SIGNING_KEYRING", "/tmp/browser-test/signing.json")
        .with_env_var("DEBUG_MODE", "false")
        .with_cmd(["-ec", "mkdir -p /tmp/browser-test; python -m server_browser_backend.auth.signing init /tmp/browser-test/signing.json; exec /usr/local/bin/server-browser-entrypoint --no-access-log"])
        .with_host_config_modifier(|config| {
            for bindings in config.port_bindings.iter_mut().flat_map(|ports| ports.values_mut()).flatten().flatten() {
                bindings.host_ip = Some("127.0.0.1".into());
            }
        })
        .with_startup_timeout(Duration::from_secs(30))
        .start().expect("start backend container (Docker is required)");
    let url = format!(
        "http://{}:{}",
        container.get_host().unwrap(),
        container.get_host_port_ipv4(8080).unwrap()
    );
    (container, url)
}
