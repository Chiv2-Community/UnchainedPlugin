//! Plugin-specific HTTP configuration; lifecycle policy belongs to the SDK.
use server_browser_client_sdk::HttpTransport;
use std::net::IpAddr;
use std::time::Duration;
use tokio::runtime::Handle;

// Runtime initialization depends on the Windows plugin globals.
#[cfg(windows)]
pub fn base_url() -> &'static str {
    &crate::tools::hook_globals::cli_args().server_browser_backend
}

#[cfg(windows)]
pub fn client() -> &'static HttpTransport {
    use crate::tools::hook_globals::cli_args;
    use std::sync::OnceLock;

    static CLIENT: OnceLock<HttpTransport> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let args = cli_args();
        let explicit_address = args.local_ip_explicit.then_some(args.local_ip.as_str());
        transport(base_url(), source_ip(args.use_ipv6, explicit_address))
            .expect("Failed to construct server-browser client")
    })
}

#[cfg(windows)]
fn source_ip(use_ipv6: bool, explicit_address: Option<&str>) -> Option<IpAddr> {
    use get_if_addrs::{get_if_addrs, IfAddr};
    use std::net::Ipv6Addr;

    if !use_ipv6 {
        return None;
    }
    let address = if let Some(address) = explicit_address {
        address.parse::<Ipv6Addr>().ok()
    } else {
        std::env::var("SERVER_ADVERTISE_IP")
            .ok()
            .and_then(|configured| configured.parse::<Ipv6Addr>().ok())
            .or_else(|| {
                get_if_addrs().ok()?.into_iter().find_map(|interface| {
                    let IfAddr::V6(address) = interface.addr else {
                        return None;
                    };
                    address.ip.is_unicast_global().then_some(address.ip)
                })
            })
    };
    if let Some(address) = address {
        sinfo!("Binding server-browser HTTP to IPv6 source {}", address);
    } else {
        swarn!("--use-ipv6 could not obtain a source address; using the OS-selected source");
    }
    address.map(IpAddr::V6)
}

fn transport(
    base_url: &str,
    source_ip: Option<IpAddr>,
) -> Result<HttpTransport, reqwest::Error> {
    let mut configuration = HttpTransport::new(base_url, source_ip)?
        .configuration()
        .clone();
    configuration.user_agent = Some("UnchainedPlugin/server-browser-api".into());
    Ok(HttpTransport::from_configuration(configuration))
}

/// Legacy native admission cannot await. Never block inside a Tokio context.
/// Timeout/task failure/shutdown keeps the banlist fail-open, not an authentication gate.
pub fn check_banned_sync(client: &HttpTransport, address: &str, runtime: &Handle) -> Option<bool> {
    if Handle::try_current().is_ok() {
        return None;
    }
    let client = client.clone();
    let address = address.to_owned();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let task = runtime.spawn(async move {
        let result =
            server_browser_client::apis::api_v1_api::check_banned(client.configuration(), &address)
                .await
                .ok()
                .map(|response| response.banned);
        let _ = sender.send(result);
    });
    let result = receiver.recv_timeout(Duration::from_secs(3)).ok().flatten();
    task.abort();
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn transport_sets_plugin_user_agent() {
        let client = super::transport("http://127.0.0.1:18080", None).unwrap();
        assert_eq!(
            client.configuration().user_agent.as_deref(),
            Some("UnchainedPlugin/server-browser-api")
        );
    }
}
