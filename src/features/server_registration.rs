use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::net::IpAddr;
use get_if_addrs::{get_if_addrs, IfAddr};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use a2s::A2SClient;
use a2s::info::Info;
use crate::events::models::{ServerStatus, GameEvent};
use crate::features::events::EVENT_SYSTEM;
use crate::features::Mod;

use crate::tools::hook_globals::cli_args;
use crate::globals;

// --- Configuration Constants ---
const MAX_DISCOVERY_TIMEOUT_SECS: u64 = 60; // Max time to wait for A2S on start
const MIN_HEARTBEAT_FLOOR_SECS: u64 = 5;   // Don't heartbeat faster than this
const MAX_HEARTBEAT_CAP_SECS: u64 = 30;     // Don't heartbeat slower than this
const HEARTBEAT_GRACE_PERIOD_SECS: u64 = 5;
const BACKOFF_INITIAL_SECS: u64 = 10;
const BACKOFF_MAX_SECS: u64 = 300;          // 5 minutes
const A2S_RETRIES: u8 = 3;

const API_BASE_PATH: &str = "/api/v1/servers";
const AUTH_HEADER: &str = "X-CHIV2-SERVER-BROWSER-KEY";
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


#[derive(Debug, Serialize)]
struct RegisterRequest<'a> {
    ports: Ports,
    name: &'a str,
    description: String,
    password_protected: bool,
    current_map: &'a str,
    player_count: i32,
    max_players: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_ip_address: Option<&'a str>,
    mods: Vec<Mod>,
}

// #[derive(Debug, Serialize, Clone)]
// pub struct Mod {
//     pub name: String,
//     pub organization: String,
//     pub version: String,
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Ports {
    game: u16,
    ping: u16,
    a2s: u16,
}

#[derive(Debug, Deserialize, Clone)]
struct RegisterResponse {
    server: RegisteredServer,
    key: String,
    refresh_before: f64,
}

#[derive(Debug, Deserialize, Clone)]
struct HeartbeatResponse {
    refresh_before: f64,
    server: RegisteredServer,
}

#[derive(Debug, Deserialize, Clone)]
struct RegisteredServer {
    unique_id: String,
    is_verified: bool,
}

#[derive(Clone, Debug)]
struct Identity {
    id: String,
    key: String,
    refresh_at: u64,
    refresh_period: u64,
}

#[derive(Debug, Serialize)]
struct UpdatePayload<'a> {
    player_count: i32,
    max_players: i32,
    #[serde(rename = "current_map")]
    map_name: &'a str,
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum WorkerState {
    Idle,
    Running,
    StopRequested,
}

// --- API Helper ---

struct BackendApi;
impl BackendApi {
    fn base_url() -> String {
        cli_args().server_browser_backend.clone().unwrap_or_default()
    }

    fn register() -> String {
        format!("{}{}", Self::base_url(), API_BASE_PATH)
    }

    fn server(id: &str) -> String {
        format!("{}/{}/{}", Self::base_url(), API_BASE_PATH, id)
    }

    fn heartbeat(id: &str) -> String {
        format!("{}/heartbeat", Self::server(id))
    }
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
    mods: Mutex<Vec<Mod>>,
    cvar: Condvar,
    http: Client,
}

fn discover_global_ipv6() -> Option<String> {
    // The game may run inside a container/Wine namespace where interface
    // enumeration does not expose the host's global address. The launcher
    // therefore supplies SERVER_ADVERTISE_IP as the preferred stable source;
    // this discovery path is the fallback for native IPv6 environments.
    get_if_addrs().ok()?.into_iter().find_map(|interface| {
        let IfAddr::V6(address) = interface.addr else { return None; };
        let ip = address.ip;
        if !ip.is_loopback() && !ip.is_unspecified() && !ip.is_unicast_link_local() {
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
                        swarn!(fn; "--use-ipv6 could not discover or obtain SERVER_ADVERTISE_IP; using 127.0.0.1");
                        "127.0.0.1".to_string()
                    }),
            }
        } else {
            local_ip.map(str::to_string).unwrap_or_default()
        };

        if let Some(address) = advertised_local_ip.as_deref() {
            sinfo!(fn; "Advertising server-browser registration with local address {}", address);
        } else if use_ipv6 {
            sinfo!(fn; "Using discovered IPv6 source {} without advertising local_ip_address", selected_source_ip);
        }

        let bind_address = if use_ipv6 {
            match selected_source_ip.parse::<IpAddr>() {
                Ok(IpAddr::V6(address)) => Some(IpAddr::V6(address)),
                _ => None,
            }
        } else {
            None
        };

        let mut http_builder = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5));
        if let Some(address) = bind_address {
            sinfo!(fn; "Binding server-browser registration HTTP to IPv6 source {}", address);
            http_builder = http_builder.local_address(Some(address));
        }
        let http = http_builder.build()
            .unwrap_or_default();

        Self {
            inner: Arc::new(RegistrationInner {
                server_addr: format_host_port(ip, query_port),
                local_ip_address: advertised_local_ip,
                state: Mutex::new(WorkerState::Idle),
                identity: Mutex::new(None),
                mods: Mutex::new(Vec::new()),
                cvar: Condvar::new(),
                http,
            }),
            handle: Mutex::new(None),
        }
    }

    /// Sets the mod list to be sent during the next registration/update.
    pub fn set_mods(&self, mods: Vec<Mod>) {
        let mut mod_lock = self.inner.mods.lock().unwrap();
        *mod_lock = mods;
        // Optionally trigger an update if we want mods to update mid-session
        self.trigger_update();
    }

    /// Spawns the worker thread. If an identity exists and is still valid, 
    /// it resumes heartbeats immediately without the discovery delay.
    pub fn start(self: Arc<Self>) {
        let mut state_lock = self.inner.state.lock().unwrap();
        if *state_lock == WorkerState::Running { return; }
        *state_lock = WorkerState::Running;

        let inner = Arc::clone(&self.inner);
        let mut handle_lock = self.handle.lock().unwrap();
        
        *handle_lock = Some(thread::spawn(move || {
            inner.run_worker();
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
            let res = self.inner.http.delete(BackendApi::server(&ident.id))
                .header(AUTH_HEADER, &ident.key)
                .send();
            
            if DEBUG_LOGGING {
                match res {
                    Ok(r) if r.status().is_success() => sinfo!(fn; "Successfully deregistered: {}", ident.id),
                    _ => swarn!(fn; "Failed to deregister: {}", ident.id),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_host_port;

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

impl RegistrationInner {
    fn run_worker(&self) {
        let a2s = A2SClient::new().expect("Failed to create A2S Client");
        let mut backoff_secs = BACKOFF_INITIAL_SECS;

        loop {
            if self.is_stopping() { break; }

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
                },
                None => {
                    if DEBUG_LOGGING { sinfo!(fn; "Registration failed. Backing off {}s", backoff_secs); }
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
                Err(true) => { // Fatal Auth Error
                    serror!(fn; "Authorization failed. Clearing identity.");
                    let mut lock = self.identity.lock().unwrap();
                    *lock = None;
                    break;
                }
                Err(false) => { // Transient Network/Backend Error
                    self.wait_interruptible(Duration::from_secs(backoff_secs));
                    backoff_secs = (backoff_secs * 2).min(BACKOFF_MAX_SECS);
                }
            }
        }
    }

    /// Polls A2S until the server responds, confirming it is ready to be registered.
    fn ensure_server_ready(&self, a2s: &A2SClient) -> bool {
        if self.identity.lock().unwrap().is_some() { return true; }

        let start = Instant::now();
        while start.elapsed().as_secs() < MAX_DISCOVERY_TIMEOUT_SECS {
            if self.is_stopping() { return false; }
            if a2s.info(&self.server_addr).is_ok() { 
                if DEBUG_LOGGING { sinfo!(fn; "Server detected on {} after {}s", self.server_addr, start.elapsed().as_secs()); }
                return true; 
            }
            thread::sleep(Duration::from_secs(2));
        }
        swarn!(fn; "A2S Discovery timeout: Server not responding at {}", self.server_addr);
        false
    }

    fn get_valid_identity(&self, a2s: &A2SClient) -> Option<Identity> {
        let mut lock = self.identity.lock().unwrap();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        
        if let Some(id) = lock.as_ref() {
            if id.refresh_at > (now + HEARTBEAT_GRACE_PERIOD_SECS) { 
                return Some(id.clone()); 
            }
            if DEBUG_LOGGING { sinfo!(fn; "Identity expiring soon (or expired), re-registering..."); }
        }

        // Register fresh
        match self.register(a2s) {
            Ok(new_id) => {
                *lock = Some(new_id.clone());
                Some(new_id)
            },
            Err(_) => None,
        }
    }
    fn register(&self, a2s: &A2SClient) -> Result<Identity, ()> {
        let info = self.poll_a2s_with_retry(a2s)?;
        let args = &cli_args();
        let description = format!("{} (build {})\n{} server", info.game, info.version, info.folder);
        let mods;
        let mut all_mods: Vec<Mod> = Vec::new();
        #[cfg(feature="mod_management")]
        {              
            use std::sync::mpsc;
            use crate::{resolvers::unchained_integration::run_on_game_thread};
            thread::sleep(Duration::from_millis(5000)); // sleep until mods are actually loaded

            let (tx, rx) = mpsc::channel();
            run_on_game_thread(move || {
                if let Some(mm) = globals().mod_manager.lock().unwrap().as_ref() {
                    mm.scan_asset_registry();
                    let _ = tx.send(mm.get_active_mod_metadata());
                }
            }); 

            let active_mods = rx.recv().unwrap_or_default();
            let mut lock = self.mods.lock().unwrap();
            *lock = active_mods.clone();
            mods = active_mods;

            if let Some(mm) = globals().mod_manager.lock().unwrap().as_ref() {
                all_mods = mm.get_available().values().cloned().collect();
            }
        }

        #[cfg(not(feature = "mod_management"))]
        {
            mods = self.mods.lock().unwrap().clone();
        }
        let server_name = match cli_args().find_ini_value(&[("Game", "[/Script/TBL.TBLGameMode]", "ServerName")]) {
            Some(name_str) => name_str,
            _ => &info.name
        };

        let (max_players, player_count) = Self::players_from_info(&info);

        let request = RegisterRequest {
            ports: Ports {
                game: info.extended_server_info.port.unwrap_or(7777),
                ping: args.game_server_ping_port.unwrap_or(0),
                a2s: args.game_server_query_port.unwrap_or(0),
            },
            name: server_name,
            description,
            password_protected: args.server_password.is_some(),
            current_map: &info.map,
            player_count: player_count as i32,
            max_players: max_players as i32,
            local_ip_address: self.local_ip_address.as_deref(),
            mods,
        };
        //sinfo!(fn; "Request: {:#?}", request);

        let res = self.http.post(BackendApi::register()).json(&request).send().map_err(|_| ())?;
        if res.status().is_success() {
            EVENT_SYSTEM.game_event_publisher.publish(GameEvent::ServerStatusEvent(ServerStatus {
                name: request.name.into(),
                description: request.description.into(),
                password_protected: request.password_protected,
                current_map: request.current_map.into(),
                player_count: request.player_count,
                max_players: request.max_players,
                mods: all_mods,
                active_mods: request.mods,
            }));
            let data: RegisterResponse = res.json().map_err(|_| ())?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            let expiry = data.refresh_before as u64;
            
            if DEBUG_LOGGING {
                sinfo!(fn; "Registered: {} | Verified: {} | Key: {}...",
                    data.server.unique_id, data.server.is_verified, &data.key[..8]);
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
        } else {
            Err(())
        }
    }

    /// Extracts player count and max players from server info, adjusting for headless mode.
    /// When headless, the server host is still counted as a player in the game, even though
    /// they do not exist.
    fn players_from_info(info: &Info) -> (u8, u8) {
        let (max_players, player_count) = if cli_args().is_headless {
            (info.max_players.saturating_sub(1), info.players.saturating_sub(1))
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
        let payload = UpdatePayload {
            player_count: player_count as i32,
            max_players: max_players as i32,
            map_name: &info.map,
        };
        let upd_res = self.http.put(BackendApi::server(&ident.id))
            .header(AUTH_HEADER, &ident.key)
            .json(&payload).send();

        if let Ok(r) = upd_res {
            let s = r.status().as_u16();
            if s == 401 || s == 403 { return Err(true); }
        }

        // 2. Heartbeat Pulse (POST)
        let hb_res = self.http.post(BackendApi::heartbeat(&ident.id))
            .header(AUTH_HEADER, &ident.key)
            .send();

        match hb_res {
            Ok(r) if r.status().is_success() => {
                // Parse the response to get the new refresh_before timestamp
                if let Ok(data) = r.json::<HeartbeatResponse>() {
                    if DEBUG_LOGGING { 
                        sdebug!(fn; "HB Success [id: {}, lat: {}ms, verified: {}, expiry: {}]",
                            ident.id, start_time.elapsed().as_millis(), data.server.is_verified, data.refresh_before); 
                    }
                    Ok(data.refresh_before as u64)
                } else {
                    // Success, but couldn't parse JSON. Return current refresh_at to keep it alive.
                    Ok(ident.refresh_at)
                }
            }
            Ok(r) if r.status().as_u16() == 401 || r.status().as_u16() == 403 => Err(true),
            _ => Err(false),
        }
    }

    fn poll_a2s_with_retry(&self, a2s: &A2SClient) -> Result<a2s::info::Info, ()> {
        for i in 0..A2S_RETRIES {
            match a2s.info(&self.server_addr) {
                Ok(info) => return Ok(info),
                Err(e) => {
                    if i == A2S_RETRIES - 1 {
                        serror!(fn; "A2S failed after {} attempts: {}", A2S_RETRIES, e);
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
