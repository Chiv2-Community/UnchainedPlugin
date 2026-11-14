use crate::events::models::{GameEvent, ServerStatus};
use crate::features::events::EVENT_SYSTEM;
use crate::features::Mod;
use a2s::info::Info;
use a2s::A2SClient;
use get_if_addrs::{get_if_addrs, IfAddr};
use server_browser_client::models::{
    Chivalry2Ports, ServerRegistrationRequest, UpdateRegisteredServer,
};
use server_browser_client_sdk::{Event, HttpTransport, Policy, Registration as SdkRegistration};
use std::net::IpAddr;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use crate::globals;
use crate::tools::hook_globals::cli_args;

// --- Configuration Constants ---
const METADATA_POLL_INTERVAL: Duration = Duration::from_secs(5);
const A2S_RETRIES: u8 = 3;

/// Format a socket endpoint for APIs that expect `host:port`.
///
/// The registration JSON carries `local_ip_address` separately from its port,
/// so that field must remain a raw IPv6 address without brackets. Brackets are
/// required only when the address and port are combined for A2S or a URL.
fn format_host_port(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

// --- Data Structures ---

#[derive(Debug, Default)]
struct WorkerState {
    stopping: bool,
    update_requested: bool,
}

// --- Main Implementation ---

/// Manages the background lifecycle of a game server registration.
#[derive(Debug)]
pub struct Registration {
    inner: Arc<RegistrationInner>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

#[derive(Debug)]
struct RegistrationInner {
    server_addr: String,
    local_ip_address: Option<String>,
    state: Mutex<WorkerState>,
    registration: Mutex<Option<SdkRegistration<HttpTransport>>>,
    cvar: Condvar,
    backend: HttpTransport,
}

fn discover_global_ipv6() -> Option<String> {
    // The game may run inside a container/Wine namespace where interface
    // enumeration does not expose the host's global address. The launcher
    // therefore supplies SERVER_ADVERTISE_IP as the preferred stable source;
    // this discovery path is the fallback for native IPv6 environments.
    get_if_addrs().ok()?.into_iter().find_map(|interface| {
        let IfAddr::V6(address) = interface.addr else {
            return None;
        };
        let ip = address.ip;
        if ip.is_unicast_global() {
            Some(ip.to_string())
        } else {
            None
        }
    })
}

impl Registration {
    /// Initializes the registration manager.
    pub fn new(ip: &str, query_port: u16, local_ip: Option<&str>, use_ipv6: bool) -> Self {
        let advertised_local_ip = local_ip
            .filter(|address| !address.is_empty())
            .map(str::to_string);
        let selected_source_ip = if use_ipv6 {
            match local_ip {
                Some(address) => address.to_string(),
                None => std::env::var("SERVER_ADVERTISE_IP")
                    .ok()
                    .and_then(|configured| match configured.parse::<IpAddr>() {
                        Ok(IpAddr::V6(_)) => Some(configured),
                        _ => None,
                    })
                    .or_else(discover_global_ipv6)
                    .unwrap_or_else(|| {
                        swarn!("--use-ipv6 could not discover or obtain SERVER_ADVERTISE_IP; using 127.0.0.1");
                        "127.0.0.1".to_string()
                    }),
            }
        } else {
            local_ip.map(str::to_string).unwrap_or_default()
        };

        if let Some(address) = advertised_local_ip.as_deref() {
            sinfo!(
                "Advertising server-browser registration with local address {}",
                address
            );
        } else if use_ipv6 {
            sinfo!(
                "Using discovered IPv6 source {} without advertising local_ip_address",
                selected_source_ip
            );
        }

        let bind_address = if use_ipv6 {
            match selected_source_ip.parse::<IpAddr>() {
                Ok(IpAddr::V6(address)) => Some(IpAddr::V6(address)),
                _ => None,
            }
        } else {
            None
        };

        if let Some(address) = bind_address {
            sinfo!(
                "Binding server-browser registration HTTP to IPv6 source {}",
                address
            );
        }
        let backend = crate::backend::transport(
            cli_args()
                .server_browser_backend
                .as_deref()
                .unwrap_or_default(),
            bind_address,
        )
        .expect("Failed to construct bounded server-browser client");

        Self {
            inner: Arc::new(RegistrationInner {
                server_addr: format_host_port(ip, query_port),
                local_ip_address: advertised_local_ip,
                state: Mutex::new(WorkerState::default()),
                registration: Mutex::new(None),
                cvar: Condvar::new(),
                backend,
            }),
            handle: Mutex::new(None),
        }
    }

    /// Wake metadata collection; the mod manager remains the source of truth.
    pub fn set_mods(&self, _mods: Vec<Mod>) {
        self.trigger_update();
    }

    /// Spawn one worker; a paused SDK manager retains its existing identity.
    pub fn start(self: Arc<Self>) {
        let mut handle = self.handle.lock().unwrap();
        if handle.is_some() {
            return;
        }
        *self.inner.state.lock().unwrap() = WorkerState::default();
        let inner = Arc::clone(&self.inner);
        *handle = Some(thread::spawn(move || {
            inner.run_reg_worker();
        }));
    }

    /// Forces the worker to wake up and perform an update (e.g., on map change).
    pub fn trigger_update(&self) {
        self.inner.state.lock().unwrap().update_requested = true;
        self.inner.cvar.notify_all();
    }

    /// Stops the worker thread and optionally deregisters from the backend.
    pub fn stop(&self, deregister: bool) {
        // Serialize start/stop through the join so two workers cannot coexist.
        let mut handle = self.handle.lock().unwrap();
        {
            let mut state = self.inner.state.lock().unwrap();
            state.stopping = true;
        }
        self.inner.cvar.notify_all();

        if let Some(handle) = handle.take() {
            let _ = handle.join();
        }

        if deregister {
            if let Some(mut registration) = self.inner.registration.lock().unwrap().take() {
                if let Err(error) = registration.stop() {
                    swarn!("Server-browser cleanup failed: {}", error);
                }
            }
        }
    }
}

impl RegistrationInner {
    fn run_reg_worker(&self) {
        let a2s = A2SClient::new().expect("Failed to create A2S Client");
        let mut registration = self.registration.lock().unwrap().take();
        while !self.is_stopping() {
            let status = match self.poll_a2s_with_retry(&a2s) {
                Ok(info) => {
                    let (request, status) = self.metadata(&info);
                    if let Some(registration) = registration.as_mut() {
                        if let Err(error) = registration.set_update(UpdateRegisteredServer {
                            player_count: request.player_count,
                            max_players: request.max_players,
                            current_map: request.current_map,
                            mods: request.mods,
                        }) {
                            swarn!("Server-browser lifecycle halted: {}", error);
                            break;
                        }
                    } else {
                        registration = Some(
                            SdkRegistration::new(self.backend.clone(), request, Policy::default())
                                .expect("Invalid default registration policy"),
                        );
                    }
                    Some(status)
                }
                Err(()) => None,
            };
            if self.is_stopping() {
                break;
            }
            if let Some(registration) = registration.as_mut() {
                match registration.poll() {
                    Ok(Event::Registered { server_id }) => {
                        sinfo!("Registered server-browser identity: {}", server_id);
                        if let Some(status) = status {
                            EVENT_SYSTEM
                                .game_event_publisher
                                .publish(GameEvent::ServerStatusEvent(status));
                        }
                    }
                    Ok(Event::RegistrationLost) => {
                        swarn!("Server-browser registration expired; SDK will re-register")
                    }
                    Ok(Event::RetryScheduled(delay)) => {
                        swarn!("Server-browser retry in {}s", delay.as_secs())
                    }
                    Ok(_) => {}
                    Err(error) => {
                        // Preserve the halted manager: never turn an uncertain
                        // registration response into a fresh registration loop.
                        swarn!("Server-browser lifecycle halted: {}", error);
                        break;
                    }
                }
                let delay = registration.next_delay().unwrap_or(METADATA_POLL_INTERVAL);
                self.wait_interruptible(delay.min(METADATA_POLL_INTERVAL));
            } else {
                self.wait_interruptible(METADATA_POLL_INTERVAL);
            }
        }
        *self.registration.lock().unwrap() = registration;
    }

    fn metadata(&self, info: &Info) -> (ServerRegistrationRequest, ServerStatus) {
        let args = &cli_args();
        let description = format!(
            "{} (build {})\n{} server",
            info.game, info.version, info.folder
        );
        let mods;
        let all_mods: Vec<Mod>;
        #[cfg(feature = "mod_management")]
        {
            // The Asset Registry is scanned once during init (see `world_init`),
            // so the mod manager's registry is already populated here. Read the
            // active and available mods straight from it instead of re-running
            // the game-thread scan (and its settle sleep) on every registration.
            if let Some(mm) = globals().mod_manager.lock().unwrap().as_ref() {
                mods = mm.get_active_mod_metadata();
                all_mods = mm.get_available().values().cloned().collect();
            } else {
                mods = Vec::new();
                all_mods = Vec::new();
            }
        }

        #[cfg(not(feature = "mod_management"))]
        {
            mods = Vec::new();
            all_mods = Vec::new();
        }

        let server_name =
            match cli_args().find_ini_value(&[("Game", "[/Script/TBL.TBLGameMode]", "ServerName")])
            {
                Some(name_str) => name_str,
                _ => &info.name,
            };

        let (max_players, player_count) = Self::players_from_info(&info);

        let request = ServerRegistrationRequest {
            ports: Chivalry2Ports {
                game: info.extended_server_info.port.unwrap_or(7777) as i32,
                ping: args.game_server_ping_port.unwrap_or(0) as i32,
                a2s: args.game_server_query_port.unwrap_or(0) as i32,
            },
            name: server_name.to_owned(),
            description,
            password_protected: Some(args.server_password.is_some()),
            current_map: info.map.clone(),
            player_count: player_count as i32,
            max_players: max_players as i32,
            local_ip_address: self.local_ip_address.clone(),
            mods: Some(
                mods.iter()
                    .map(|m| {
                        server_browser_client::models::Mod::new(
                            m.name.clone(),
                            m.organization.clone(),
                            m.version.clone(),
                        )
                    })
                    .collect(),
            ),
        };
        let status = ServerStatus {
            name: request.name.clone().into(),
            description: request.description.clone().into(),
            password_protected: request.password_protected.unwrap_or(false),
            current_map: request.current_map.clone().into(),
            player_count: request.player_count,
            max_players: request.max_players,
            mods: all_mods,
            active_mods: mods,
        };
        (request, status)
    }

    /// Extracts player count and max players from server info, adjusting for headless mode.
    /// When headless, the server host is still counted as a player in the game, even though
    /// they do not exist.
    fn players_from_info(info: &Info) -> (u8, u8) {
        let (max_players, player_count) = if cli_args().is_headless {
            (
                info.max_players.saturating_sub(1),
                info.players.saturating_sub(1),
            )
        } else {
            (info.max_players, info.players)
        };
        (max_players, player_count)
    }

    fn poll_a2s_with_retry(&self, a2s: &A2SClient) -> Result<a2s::info::Info, ()> {
        for i in 0..A2S_RETRIES {
            if self.is_stopping() {
                return Err(());
            }
            match a2s.info(&self.server_addr) {
                Ok(info) => return Ok(info),
                Err(e) => {
                    if i == A2S_RETRIES - 1 {
                        serror!("A2S failed after {} attempts: {}", A2S_RETRIES, e);
                        return Err(());
                    }
                    self.wait_interruptible(Duration::from_millis(500));
                }
            }
        }
        Err(())
    }

    fn wait_interruptible(&self, duration: Duration) {
        let state = self.state.lock().unwrap();
        let (mut state, _) = self
            .cvar
            .wait_timeout_while(state, duration, |state| {
                !state.stopping && !state.update_requested
            })
            .unwrap();
        state.update_requested = false;
    }

    fn is_stopping(&self) -> bool {
        self.state.lock().unwrap().stopping
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_ipv4_endpoint_without_brackets() {
        assert_eq!(format_host_port("127.0.0.1", 7071), "127.0.0.1:7071");
    }

    #[test]
    fn formats_ipv6_endpoint_with_brackets() {
        assert_eq!(
            format_host_port("2a0c:d242:903b:ba00::ff99", 7071),
            "[2a0c:d242:903b:ba00::ff99]:7071"
        );
    }

    #[test]
    fn preserves_already_bracketed_ipv6_endpoint() {
        assert_eq!(
            format_host_port("[2a0c:d242:903b:ba00::ff99]", 7071),
            "[2a0c:d242:903b:ba00::ff99]:7071"
        );
    }
}
