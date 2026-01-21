use crate::serror;
use crate::sinfo;
use crate::tools::hook_globals::globals;
use easy_upnp::{add_ports, delete_ports, Ipv4Cidr, PortMappingProtocol, UpnpConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use once_cell::sync::Lazy;

#[cfg(feature = "upnp")]
static UPNP_STOP_FLAG: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

#[cfg(feature = "upnp")]
pub fn stop_upnp_manager() {
    UPNP_STOP_FLAG.store(true, Ordering::Relaxed);
}

#[cfg(feature = "upnp")]
pub fn get_configs() -> Vec<UpnpConfig> {
    let args = &globals().cli_args;
    let mut configs = Vec::new();

    let ipv4Cidr = args.local_ip.as_ref().map(|ip| {
        Ipv4Cidr::from_str(ip).map_err(|e| {
            serror!("Failed to parse local IP address '{}': {}", ip, e);
            e
        })
    });

    if let Some(Ok(ipv4Cidr)) = ipv4Cidr {
        if let Some(port) = args.game_port {
            configs.push(UpnpConfig {
                address: Some(ipv4Cidr),
                port,
                protocol: PortMappingProtocol::UDP,
                duration: 300,
                comment: "Chivalry 2 Unchained Game Port".to_string(),
            });
        }

        if let Some(port) = args.game_server_ping_port {
            configs.push(UpnpConfig {
                address: Some(ipv4Cidr),
                port,
                protocol: PortMappingProtocol::UDP,
                duration: 300,
                comment: "Chivalry 2 Unchained Ping Port".to_string(),
            });
        }
    }

    configs
}

#[cfg(feature = "upnp")]
pub fn run_upnp_manager() {
    sinfo!("UPnP manager started");

    loop {
        if UPNP_STOP_FLAG.load(Ordering::Relaxed) {
            break;
        }

        let configs = get_configs();
        if configs.is_empty() {
            sinfo!("No ports to forward via UPnP");
        } else {
            let results: Vec<_> = add_ports(configs).collect();
            let mut any_error = false;

            for result in &results {
                if let Err(err) = result {
                    serror!("Failed to forward port: {}", err);
                    any_error = true;
                }
            }

            if !any_error {
                sinfo!("Successfully forwarded all ports");
            } else {
                serror!("Some ports failed to forward");
            }
        }

        // Refresh every 4 minutes (duration is 300s = 5m)
        for _ in 0..240 {
            if UPNP_STOP_FLAG.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    sinfo!("UPnP manager stopping, unregistering ports");
    let configs = get_configs();
    if !configs.is_empty() {
        let results: Vec<_> = delete_ports(configs).collect();
        let mut any_error = false;

        for result in &results {
            if let Err(err) = result {
                serror!("Failed to remove port: {}", err);
                any_error = true;
            }
        }

        if !any_error {
            sinfo!("Successfully un-forwarded all ports");
        } else {
            serror!("Some ports failed to un-forward");
        }
    }
}

