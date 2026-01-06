use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use a2s::A2SClient;

use crate::{globals, serror, sinfo, swarn};

// --- Constants ---
const INITIAL_DELAY_SECS: u64 = 20;
const MAX_HEARTBEAT_SECS: u64 = 60;
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

#[derive(Clone)]
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

#[derive(PartialEq)]
enum WorkerState {
    Idle,
    Running,
    StopRequested,
}

// --- API Helpers ---

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

pub struct Registration {
    inner: Arc<RegistrationInner>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

struct RegistrationInner {
    server_addr: String,
    state: Mutex<WorkerState>,
    identity: Mutex<Option<Identity>>,
    cvar: Condvar,
    http: Client,
}

impl Registration {
    /// Creates a new registration manager but does not start any threads.
    pub fn new(ip: &str, query_port: u16) -> Self {
        Self {
            inner: Arc::new(RegistrationInner {
                server_addr: format!("{ip}:{query_port}"),
                state: Mutex::new(WorkerState::Idle),
                identity: Mutex::new(None),
                cvar: Condvar::new(),
                http: Client::new(),
            }),
            handle: Mutex::new(None),
        }
    }

    /// Spawns the background heartbeat thread. 
    /// If an existing valid identity is found, it resumes immediately. 
    /// Otherwise, it waits 20 seconds before initial registration.
    pub fn start(self: Arc<Self>) {
        let mut state_lock = self.inner.state.lock().unwrap();
        if *state_lock == WorkerState::Running { return; }
        *state_lock = WorkerState::Running;

        let inner = Arc::clone(&self.inner);
        let mut handle_lock = self.handle.lock().unwrap();
        
        *handle_lock = Some(thread::spawn(move || {
            let resume = {
                let id_lock = inner.identity.lock().unwrap();
                id_lock.as_ref().map_or(false, |id| {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    id.refresh_at > now
                })
            };

            if !resume {
                if DEBUG_LOGGING { sinfo!(f; "Waiting {}s for server initialization...", INITIAL_DELAY_SECS); }
                let (lock, cvar) = (&inner.state, &inner.cvar);
                let mut state = lock.lock().unwrap();
                let _ = cvar.wait_timeout(state, Duration::from_secs(INITIAL_DELAY_SECS)).unwrap();
            } else if DEBUG_LOGGING {
                sinfo!(f; "Resuming heartbeat with existing identity.");
            }
            
            inner.run_loop();
        }));
    }

    /// Forces the heartbeat thread to wake up and send an update immediately.
    /// Useful for player join/leave events or map changes.
    pub fn trigger_update(&self) {
        self.inner.cvar.notify_all();
    }

    /// Stops the background worker thread.
    /// - `deregister`: If true, sends a DELETE request to the backend.
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
            if DEBUG_LOGGING { sinfo!(f; "Server {} deregistered.", ident.id); }
        }
    }
}

impl RegistrationInner {
    /// Internal worker loop that manages registration and heartbeats.
    fn run_loop(&self) {
        let a2s = A2SClient::new().expect("Failed to create A2S Client");
        
        loop {
            if self.is_stopping() { break; }

            // 1. Check or Create Identity
            let ident = match self.get_valid_identity(&a2s) {
                Some(id) => id,
                None => {
                    thread::sleep(Duration::from_secs(10));
                    continue;
                }
            };

            // 2. Perform Update & Heartbeat
            if let Err(fatal) = self.send_heartbeat_and_update(&a2s, &ident) {
                if fatal {
                    let mut lock = self.identity.lock().unwrap();
                    *lock = None;
                    break; 
                }
            }

            // 3. Wait for interval or signal
            let state_lock = self.state.lock().unwrap();
            let result = self.cvar.wait_timeout(state_lock, Duration::from_secs(ident.refresh_period)).unwrap();
            if *result.0 == WorkerState::StopRequested { break; }
        }
    }

    fn get_valid_identity(&self, a2s: &A2SClient) -> Option<Identity> {
        let mut lock = self.identity.lock().unwrap();
        
        if let Some(id) = lock.as_ref() {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            if id.refresh_at > now {
                return Some(id.clone());
            }
        }

        match self.register(a2s) {
            Ok(new_id) => {
                *lock = Some(new_id.clone());
                Some(new_id)
            },
            Err(_) => None,
        }
    }

    fn register(&self, a2s: &A2SClient) -> Result<Identity, ()> {
        let info = a2s.info(&self.server_addr).map_err(|e| serror!(f; "A2S Error: {}", e))?;
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

        let res = self.http.post(BackendApi::register()).json(&request).send();
        match res {
            Ok(r) if r.status().is_success() => {
                let data: RegisterResponse = r.json().map_err(|_| ())?;
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                let expiry = data.refresh_before as u64;
                
                let period = if expiry > now {
                    ((expiry - now) / 3).clamp(10, MAX_HEARTBEAT_SECS)
                } else {
                    10
                };

                Ok(Identity {
                    id: data.server.unique_id,
                    key: data.key,
                    refresh_at: expiry,
                    refresh_period: period,
                })
            }
            _ => Err(()),
        }
    }

    /// Sends both the metadata update (PUT) and the heartbeat pulse (POST).
    /// Returns Err(true) if the failure is a fatal auth error.
    fn send_heartbeat_and_update(&self, a2s: &A2SClient, ident: &Identity) -> Result<(), bool> {
        let info = a2s.info(&self.server_addr).map_err(|_| false)?;

        // Metadata Update
        let payload = UpdatePayload {
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            map_name: &info.map,
        };
        let upd = self.http.put(BackendApi::server(&ident.id))
            .header(AUTH_HEADER, &ident.key)
            .json(&payload).send();

        if let Ok(r) = upd {
            let status = r.status().as_u16();
            if status == 401 || status == 403 { return Err(true); }
        }

        // Heartbeat Pulse
        let hb = self.http.post(BackendApi::heartbeat(&ident.id))
            .header(AUTH_HEADER, &ident.key)
            .send();

        match hb {
            Ok(r) if r.status().is_success() => {
                if DEBUG_LOGGING { sinfo!(f; "Heartbeat/Update sent for {}", ident.id); }
                Ok(())
            }
            Ok(r) if r.status().as_u16() == 401 || r.status().as_u16() == 403 => Err(true),
            _ => Err(false),
        }
    }

    fn is_stopping(&self) -> bool {
        *self.state.lock().unwrap() == WorkerState::StopRequested
    }
}