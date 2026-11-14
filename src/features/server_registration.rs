use crate::events::models::{GameEvent, ServerStatus};
use crate::features::events::EVENT_SYSTEM;
use crate::features::Mod;
use a2s::info::Info;
use a2s::A2SClient;
use get_if_addrs::{get_if_addrs, IfAddr};
use std::net::IpAddr;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use unchained_backend::{
    apis::api_v1_api,
    invalidates_registration,
    models::{Chivalry2Ports, ServerRegistrationRequest, UpdateRegisteredServer},
    BackendClient,
};

use crate::globals;
use crate::tools::hook_globals::cli_args;

// --- Configuration Constants ---
const MAX_DISCOVERY_TIMEOUT_SECS: u64 = 60; // Max time to wait for A2S on start
const MIN_HEARTBEAT_FLOOR_SECS: u64 = 5; // Don't heartbeat faster than this
const MAX_HEARTBEAT_CAP_SECS: u64 = 30; // Don't heartbeat slower than this
const HEARTBEAT_GRACE_PERIOD_SECS: u64 = 5;
const BACKOFF_INITIAL_SECS: u64 = 10;
const BACKOFF_MAX_SECS: u64 = 300; // 5 minutes
const A2S_RETRIES: u8 = 3;

const DEBUG_LOGGING: bool = true;

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

#[derive(Clone)]
struct Identity {
    id: String,
    key: String,
    refresh_at: u64,
    refresh_period: u64,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum WorkerState {
    Idle,
    Running,
    StopRequested,
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
    identity: Mutex<Option<Identity>>,
    cvar: Condvar,
    backend: BackendClient,
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
        let backend = BackendClient::new(
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
                state: Mutex::new(WorkerState::Idle),
                identity: Mutex::new(None),
                cvar: Condvar::new(),
                backend,
            }),
            handle: Mutex::new(None),
        }
    }

    /// Signals that the active mod set changed so the next registration picks
    /// it up. The mod list itself is read from the mod manager at registration
    /// time (the Asset Registry is scanned once during init), so nothing needs
    /// to be stored here.
    pub fn set_mods(&self, _mods: Vec<Mod>) {
        self.trigger_update();
    }

    /// Spawns the worker thread. If an identity exists and is still valid,
    /// it resumes heartbeats immediately without the discovery delay.
    pub fn start(self: Arc<Self>) {
        let mut state_lock = self.inner.state.lock().unwrap();
        if *state_lock == WorkerState::Running {
            return;
        }
        *state_lock = WorkerState::Running;

        let inner = Arc::clone(&self.inner);
        let mut handle_lock = self.handle.lock().unwrap();

        *handle_lock = Some(thread::spawn(move || {
            inner.run_reg_worker();
        }));
    }

    /// Forces the worker to wake up and perform an update (e.g., on map change).
    pub fn trigger_update(&self) {
        self.inner.cvar.notify_all();
    }

    /// Stops the worker thread and optionally deregisters from the backend.
    pub fn stop(&self, deregister: bool) {
        {
            let mut state = self.inner.state.lock().unwrap();
            *state = WorkerState::StopRequested;
        }
        self.inner.cvar.notify_all();

        if let Some(handle) = self.handle.lock().unwrap().take() {
            let _ = handle.join();
        }

        if deregister {
            self.perform_deregistration();
            let mut id_lock = self.inner.identity.lock().unwrap();
            *id_lock = None;
        }

        let mut state = self.inner.state.lock().unwrap();
        *state = WorkerState::Idle;
    }

    fn perform_deregistration(&self) {
        let ident_opt = self.inner.identity.lock().unwrap().clone();
        if let Some(ident) = ident_opt {
            let res = api_v1_api::delete_server(
                self.inner.backend.configuration(),
                &ident.key,
                &ident.id,
            );

            if DEBUG_LOGGING {
                match res {
                    Ok(_) => sinfo!("Successfully deregistered: {}", ident.id),
                    _ => swarn!("Failed to deregister: {}", ident.id),
                }
            }
        }
    }
}

impl RegistrationInner {
    fn run_reg_worker(&self) {
        let a2s = A2SClient::new().expect("Failed to create A2S Client");
        let mut backoff_secs = BACKOFF_INITIAL_SECS;

        loop {
            if self.is_stopping() {
                break;
            }

            // 1. Discovery/Ready Check (only if fresh start)
            if !self.ensure_server_ready(&a2s) {
                thread::sleep(Duration::from_secs(5));
                continue;
            }

            // 2. Resolve Identity
            let ident = match self.get_valid_identity(&a2s) {
                Some(id) => {
                    backoff_secs = BACKOFF_INITIAL_SECS; // Reset backoff on success
                    id
                }
                None => {
                    if DEBUG_LOGGING {
                        sinfo!("Registration failed. Backing off {}s", backoff_secs);
                    }
                    self.wait_interruptible(Duration::from_secs(backoff_secs));
                    backoff_secs = (backoff_secs * 2).min(BACKOFF_MAX_SECS);
                    continue;
                }
            };

            // 3. Perform the Heartbeat + Metadata Update
            match self.send_heartbeat_and_update(&a2s, &ident) {
                Ok(new_expiry) => {
                    backoff_secs = BACKOFF_INITIAL_SECS;

                    // Update our internal Identity with the new timestamp from the backend
                    {
                        let mut lock = self.identity.lock().unwrap();
                        if let Some(ref mut id) = *lock {
                            id.refresh_at = new_expiry;
                        }
                    }

                    self.wait_interruptible(Duration::from_secs(ident.refresh_period));
                }
                Err(true) => {
                    // Rejected or missing registration
                    swarn!("Registration rejected. Clearing identity and re-registering.");
                    let mut lock = self.identity.lock().unwrap();
                    *lock = None;
                }
                Err(false) => {
                    // Transient Network/Backend Error
                    self.wait_interruptible(Duration::from_secs(backoff_secs));
                    backoff_secs = (backoff_secs * 2).min(BACKOFF_MAX_SECS);
                }
            }
        }
    }

    /// Polls A2S until the server responds, confirming it is ready to be registered.
    fn ensure_server_ready(&self, a2s: &A2SClient) -> bool {
        if self.identity.lock().unwrap().is_some() {
            return true;
        }

        let start = Instant::now();
        while start.elapsed().as_secs() < MAX_DISCOVERY_TIMEOUT_SECS {
            if self.is_stopping() {
                return false;
            }
            if a2s.info(&self.server_addr).is_ok() {
                if DEBUG_LOGGING {
                    sinfo!(
                        "Server detected on {} after {}s",
                        self.server_addr,
                        start.elapsed().as_secs()
                    );
                }
                return true;
            }
            thread::sleep(Duration::from_secs(2));
        }
        swarn!(
            "A2S Discovery timeout: Server not responding at {}",
            self.server_addr
        );
        false
    }

    fn get_valid_identity(&self, a2s: &A2SClient) -> Option<Identity> {
        let mut lock = self.identity.lock().unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(id) = lock.as_ref() {
            if id.refresh_at > (now + HEARTBEAT_GRACE_PERIOD_SECS) {
                return Some(id.clone());
            }
            if DEBUG_LOGGING {
                sinfo!("Identity expiring soon (or expired), re-registering...");
            }
        }

        // Register fresh
        match self.register(a2s) {
            Ok(new_id) => {
                *lock = Some(new_id.clone());
                Some(new_id)
            }
            Err(_) => None,
        }
    }
    fn register(&self, a2s: &A2SClient) -> Result<Identity, ()> {
        let info = self.poll_a2s_with_retry(a2s)?;
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
                        unchained_backend::models::Mod::new(
                            m.name.clone(),
                            m.organization.clone(),
                            m.version.clone(),
                        )
                    })
                    .collect(),
            ),
        };
        //sinfo!("Request: {:#?}", request);

        let data =
            api_v1_api::register(self.backend.configuration(), request.clone()).map_err(|_| ())?;
        EVENT_SYSTEM
            .game_event_publisher
            .publish(GameEvent::ServerStatusEvent(ServerStatus {
                name: request.name.into(),
                description: request.description.into(),
                password_protected: request.password_protected.unwrap_or(false),
                current_map: request.current_map.into(),
                player_count: request.player_count,
                max_players: request.max_players,
                mods: all_mods,
                active_mods: mods,
            }));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expiry = data.refresh_before as u64;

        if DEBUG_LOGGING {
            sinfo!(
                "Registered: {} | Verified: {}",
                data.server.unique_id,
                data.server.is_verified
            );
        }

        let period = if expiry > now {
            (expiry - now).clamp(MIN_HEARTBEAT_FLOOR_SECS, MAX_HEARTBEAT_CAP_SECS)
        } else {
            MIN_HEARTBEAT_FLOOR_SECS
        };

        Ok(Identity {
            id: data.server.unique_id,
            key: data.key,
            refresh_at: expiry,
            refresh_period: period,
        })
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

    fn send_heartbeat_and_update(&self, a2s: &A2SClient, ident: &Identity) -> Result<u64, bool> {
        let info = self.poll_a2s_with_retry(a2s).map_err(|_| false)?;
        let start_time = Instant::now();

        let (max_players, player_count) = Self::players_from_info(&info);

        // 1. Metadata Update (PUT)
        let payload = UpdateRegisteredServer {
            player_count: player_count as i32,
            max_players: max_players as i32,
            current_map: info.map.clone(),
            ..Default::default()
        };
        if let Err(error) =
            api_v1_api::update(self.backend.configuration(), &ident.key, &ident.id, payload)
        {
            if invalidates_registration(&error) {
                return Err(true);
            }
        }

        // 2. Heartbeat Pulse (POST)
        let hb_res = api_v1_api::heartbeat(self.backend.configuration(), &ident.key, &ident.id);

        match hb_res {
            Ok(data) => {
                if DEBUG_LOGGING {
                    strace!(
                        "HB Success [id: {}, lat: {}ms, verified: {}, expiry: {}]",
                        ident.id,
                        start_time.elapsed().as_millis(),
                        data.server.is_verified,
                        data.refresh_before
                    );
                }
                Ok(data.refresh_before as u64)
            }
            Err(error) if invalidates_registration(&error) => Err(true),
            _ => Err(false),
        }
    }

    fn poll_a2s_with_retry(&self, a2s: &A2SClient) -> Result<a2s::info::Info, ()> {
        for i in 0..A2S_RETRIES {
            match a2s.info(&self.server_addr) {
                Ok(info) => return Ok(info),
                Err(e) => {
                    if i == A2S_RETRIES - 1 {
                        serror!("A2S failed after {} attempts: {}", A2S_RETRIES, e);
                        return Err(());
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }
        Err(())
    }

    fn wait_interruptible(&self, duration: Duration) {
        let state = self.state.lock().unwrap();
        let _ = self.cvar.wait_timeout(state, duration).unwrap();
    }

    fn is_stopping(&self) -> bool {
        *self.state.lock().unwrap() == WorkerState::StopRequested
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
