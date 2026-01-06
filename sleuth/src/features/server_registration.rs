use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use a2s::A2SClient;

use crate::{globals, serror, sinfo, swarn};

// --- Configuration Constants ---
const MAX_DISCOVERY_TIMEOUT_SECS: u64 = 60; // Max time to wait for A2S on start
const MIN_HEARTBEAT_FLOOR_SECS: u64 = 15;   // Don't heartbeat faster than this
const MAX_HEARTBEAT_CAP_SECS: u64 = 60;     // Don't heartbeat slower than this
const BACKOFF_INITIAL_SECS: u64 = 10;
const BACKOFF_MAX_SECS: u64 = 300;          // 5 minutes
const A2S_RETRIES: u8 = 3;

const API_BASE_PATH: &str = "/api/v1/servers";
const AUTH_HEADER: &str = "X-CHIV2-SERVER-BROWSER-KEY";
const DEBUG_LOGGING: bool = true;

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
    local_ip_address: &'a str,
    mods: &'a [ModInfo<'a>],
}

#[derive(Debug, Serialize)]
struct ModInfo<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Debug, Serialize)]
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
struct RegisteredServer {
    unique_id: String,
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
        globals().cli_args.server_browser_backend.clone().unwrap_or_default()
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
    state: Mutex<WorkerState>,
    identity: Mutex<Option<Identity>>,
    cvar: Condvar,
    http: Client,
}

impl Registration {
    /// Initializes the registration manager.
    pub fn new(ip: &str, query_port: u16) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            inner: Arc::new(RegistrationInner {
                server_addr: format!("{ip}:{query_port}"),
                state: Mutex::new(WorkerState::Idle),
                identity: Mutex::new(None),
                cvar: Condvar::new(),
                http,
            }),
            handle: Mutex::new(None),
        }
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
            let _ = self.inner.http.delete(BackendApi::server(&ident.id))
                .header(AUTH_HEADER, &ident.key)
                .send();
            if DEBUG_LOGGING { sinfo!(f; "Deregistered: {}", ident.id); }
        }
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
                    if DEBUG_LOGGING { sinfo!(f; "Registration failed. Backing off {}s", backoff_secs); }
                    self.wait_interruptible(Duration::from_secs(backoff_secs));
                    backoff_secs = (backoff_secs * 2).min(BACKOFF_MAX_SECS);
                    continue;
                }
            };

            // 3. Heartbeat & Update
            match self.send_heartbeat_and_update(&a2s, &ident) {
                Ok(_) => {
                    backoff_secs = BACKOFF_INITIAL_SECS;
                    self.wait_interruptible(Duration::from_secs(ident.refresh_period));
                }
                Err(true) => { // Fatal Auth Error
                    serror!(f; "Invalid credentials. Stopping worker.");
                    let mut lock = self.identity.lock().unwrap();
                    *lock = None;
                    break;
                }
                Err(false) => { // Transient Error
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
                if DEBUG_LOGGING { sinfo!(f; "Server detected on {} after {}s", self.server_addr, start.elapsed().as_secs()); }
                return true; 
            }
            thread::sleep(Duration::from_secs(2));
        }
        swarn!(f; "A2S Discovery timeout: Server not responding at {}", self.server_addr);
        false
    }

    fn get_valid_identity(&self, a2s: &A2SClient) -> Option<Identity> {
        let mut lock = self.identity.lock().unwrap();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        
        if let Some(id) = lock.as_ref() {
            if id.refresh_at > now { return Some(id.clone()); }
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
        let args = &globals().cli_args;
        let description = format!("{} (build {})\n{} server", info.game, info.version, info.folder);

        let request = RegisterRequest {
            ports: Ports {
                game: info.extended_server_info.port.unwrap_or(7777),
                ping: args.game_server_ping_port.unwrap_or(0),
                a2s: args.game_server_query_port.unwrap_or(0),
            },
            name: &info.name,
            description,
            password_protected: args.server_password.is_some(),
            current_map: &info.map,
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            local_ip_address: "127.0.0.1",
            mods: &[],
        };

        let res = self.http.post(BackendApi::register()).json(&request).send().map_err(|_| ())?;
        if res.status().is_success() {
            let data: RegisterResponse = res.json().map_err(|_| ())?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            let expiry = data.refresh_before as u64;
            
            let period = if expiry > now {
                ((expiry - now) / 3).clamp(MIN_HEARTBEAT_FLOOR_SECS, MAX_HEARTBEAT_CAP_SECS)
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

    fn send_heartbeat_and_update(&self, a2s: &A2SClient, ident: &Identity) -> Result<(), bool> {
        let info = self.poll_a2s_with_retry(a2s).map_err(|_| false)?;
        let start_time = Instant::now();

        // 1. PUT Update
        let payload = UpdatePayload {
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            map_name: &info.map,
        };
        let upd_res = self.http.put(BackendApi::server(&ident.id))
            .header(AUTH_HEADER, &ident.key)
            .json(&payload).send();

        if let Ok(r) = upd_res {
            let s = r.status().as_u16();
            if s == 401 || s == 403 { return Err(true); }
        }

        // 2. POST Heartbeat
        let hb_res = self.http.post(BackendApi::heartbeat(&ident.id))
            .header(AUTH_HEADER, &ident.key)
            .send();

        match hb_res {
            Ok(r) if r.status().is_success() => {
                if DEBUG_LOGGING { 
                    sinfo!(f; "Heartbeat/Update success [id: {}, lat: {}ms]", ident.id, start_time.elapsed().as_millis()); 
                }
                Ok(())
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
                        serror!(f; "A2S failed after {} attempts: {}", A2S_RETRIES, e);
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