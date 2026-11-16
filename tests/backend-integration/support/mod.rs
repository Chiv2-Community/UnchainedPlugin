use std::time::Duration;
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::SyncRunner,
    Container, GenericImage, ImageExt,
};

pub fn backend() -> (Container<GenericImage>, String) {
    // Signing keys and registry files exist only in this disposable container.
    let container = GenericImage::new(
        "ghcr.io/chiv2-community/c2serverbrowserbackend",
        "1.3.0-alpha-2_e820a32",
    )
        .with_entrypoint("/bin/sh")
        .with_exposed_port(8080.tcp())
        .with_wait_for(WaitFor::message_on_stderr("Uvicorn running on"))
        .with_env_var("CONFIG_DIR", "/tmp/browser-test")
        .with_env_var("ADMIN_KEY", "container-test-admin")
        .with_env_var("TRUSTED_PROXY_CIDRS", "")
        .with_env_var("DISCORD_SIGNING_KEYRING", "/tmp/browser-test/signing.json")
        .with_env_var("DEBUG_MODE", "false")
        .with_cmd([
            "-ec",
            "mkdir -p /tmp/browser-test\n\
             python -m server_browser_backend.auth.signing init /tmp/browser-test/signing.json\n\
             exec /usr/local/bin/server-browser-entrypoint --no-access-log",
        ])
        .with_host_config_modifier(|config| {
            if let Some(ports) = &mut config.port_bindings {
                for binding in ports.values_mut().flatten().flatten() {
                    binding.host_ip = Some("127.0.0.1".into());
                }
            }
        })
        .with_startup_timeout(Duration::from_secs(30))
        .start()
        .expect("start backend container (Docker is required)");
    let url = format!(
        "http://{}:{}",
        container.get_host().unwrap(),
        container.get_host_port_ipv4(8080).unwrap()
    );
    (container, url)
}
