//! Plugin-specific HTTP configuration; lifecycle policy belongs to the SDK.
use server_browser_client::apis::configuration::Configuration;
use server_browser_client_sdk::HttpTransport;
use std::net::IpAddr;
use std::time::Duration;
use tokio::runtime::Handle;

pub fn transport(
    base_url: &str,
    source_ip: Option<IpAddr>,
) -> Result<HttpTransport, reqwest::Error> {
    let transport = HttpTransport::new(base_url, source_ip)?;
    Ok(HttpTransport::from_configuration(Configuration {
        user_agent: Some("UnchainedPlugin/server-browser-api".into()),
        ..transport.configuration().clone()
    }))
}

/// Legacy native admission cannot await. Never block inside a Tokio context.
/// Timeout/panic/shutdown keeps the banlist fail-open, not an authentication gate.
pub fn check_banned_sync(base_url: &str, address: &str, runtime: &Handle) -> Option<bool> {
    if Handle::try_current().is_ok() {
        return None;
    }
    let base_url = base_url.to_owned();
    let address = address.to_owned();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let task = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.spawn(async move {
            let result = match transport(&base_url, None) {
                Ok(client) => server_browser_client::apis::api_v1_api::check_banned(
                    client.configuration(),
                    &address,
                )
                .await
                .ok()
                .map(|response| response.banned),
                Err(_) => None,
            };
            let _ = sender.send(result);
        })
    }))
    .ok()?;
    let result = receiver.recv_timeout(Duration::from_secs(3)).ok().flatten();
    task.abort();
    result
}
