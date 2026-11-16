use server_browser_client_sdk::HttpTransport;
use std::sync::OnceLock;

use crate::tools::hook_globals::cli_args;

pub fn client() -> &'static HttpTransport {
    static CLIENT: OnceLock<HttpTransport> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let args = cli_args();
        #[cfg(feature = "server_registration")]
        let source_ip = source_ip(args);
        #[cfg(not(feature = "server_registration"))]
        let source_ip = None;
        crate::backend::transport(
            args.server_browser_backend.as_deref().unwrap_or_default(),
            source_ip,
        )
        .expect("Failed to construct bounded server-browser client")
    })
}

#[cfg(feature = "server_registration")]
fn source_ip(args: &crate::tools::cli_args::CLIArgs) -> Option<std::net::IpAddr> {
    use get_if_addrs::{get_if_addrs, IfAddr};
    use std::net::{IpAddr, Ipv6Addr};

    if !args.use_ipv6 {
        return None;
    }
    let address = if args.local_ip_explicit {
        args.local_ip.parse::<Ipv6Addr>().ok()
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
